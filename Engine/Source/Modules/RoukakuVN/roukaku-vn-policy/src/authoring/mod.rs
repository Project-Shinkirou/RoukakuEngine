//! Luau authoring mechanisms. Default VN behavior and product loading are consumers.
mod api;
mod contract;
mod timeline;

pub use contract::*;

use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    ops::Bound::{Excluded, Included},
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};

use mlua::{Function, Lua, LuaSerdeExt, Table, Thread, Value, thread::ThreadStatus};
use roukaku_policy::{PolicyExecutionBudget, SandboxedLuau};
use roukaku_runtime::TaskScope;

use crate::{
    PolicyQueryContext, VnPolicyState,
    luau_budget::{HostBudget, SharedBudget},
};

static NEXT_LIVE_ID: AtomicU64 = AtomicU64::new(1);

// Live tokens remain distinct across runtime replacement and exactly
// representable when returned to Luau as segment handles.
fn live_id() -> mlua::Result<u64> {
    NEXT_LIVE_ID
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |id| {
            (id < (1_u64 << 53)).then_some(id + 1)
        })
        .map_err(|_| mlua::Error::runtime("authoring live identity exhausted"))
}

struct Shared {
    root: TaskScope,
    scope: TaskScope,
    paragraph: Option<ParagraphId>,
    story_thread: Option<usize>,
    visits: BTreeMap<String, u64>,
    records: Vec<NodeRecord>,
    recoverable: RecoverableState,
    segments: BTreeMap<u64, timeline::Segment>,
    active_segments: BTreeSet<u64>,
    cancelling: bool,
    pending: Option<InputWait>,
    budget: SharedBudget,
    limits: PolicyExecutionBudget,
}

#[derive(Clone)]
struct Context {
    shared: Rc<RefCell<Shared>>,
    modules: Table,
    host: Rc<dyn AuthoringHost>,
}

impl Context {
    fn available(&self) -> mlua::Result<()> {
        let shared = self.shared.borrow();
        shared.budget.borrow().check()?;
        if shared.root.is_cancelled() {
            return Err(mlua::Error::runtime("authoring owner scope cancelled"));
        }
        Ok(())
    }

    fn active(&self) -> mlua::Result<()> {
        let shared = self.shared.borrow();
        shared.budget.borrow().check()?;
        if shared.scope.is_cancelled() {
            return Err(mlua::Error::runtime("authoring task scope cancelled"));
        }
        Ok(())
    }

    fn resolve(&self, reference: &FunctionRef) -> mlua::Result<Function> {
        let module: Table = self.modules.get(reference.module.as_str())?;
        module.get(reference.name.as_str())
    }

    fn invoke(&self, reference: &FunctionRef) -> mlua::Result<()> {
        self.active()?;
        self.resolve(reference)?.call(())
    }

    fn charge(&self, bytes: usize) -> mlua::Result<()> {
        self.available()?;
        self.shared.borrow().budget.borrow_mut().replace(0, bytes)
    }

    /// Preflight bounds the Rust allocation before deserializing a Lua table.
    fn data<T: serde::de::DeserializeOwned>(&self, lua: &Lua, value: Value) -> mlua::Result<T> {
        let shared = self.shared.borrow();
        let bytes = shared.budget.borrow().lua_value_bytes(&value, 0)?;
        drop(shared);
        self.charge(bytes)?;
        validate_data(lua, &value, 0, self.shared.borrow().limits.snapshot_depth)?;
        lua.from_value(value)
    }
}

fn validate_data(lua: &Lua, value: &Value, depth: usize, limit: usize) -> mlua::Result<()> {
    if depth > limit {
        return Err(mlua::Error::runtime("authoring data depth exceeded"));
    }
    match value {
        Value::Nil | Value::Boolean(_) | Value::Integer(_) | Value::String(_) => Ok(()),
        Value::LightUserData(value) if value.0.is_null() => Ok(()),
        Value::Number(number) if number.is_finite() => Ok(()),
        Value::Table(table)
            if table
                .metatable()
                .is_none_or(|meta| meta.to_pointer() == lua.array_metatable().to_pointer()) =>
        {
            for pair in table.clone().pairs::<Value, Value>() {
                let (key, value) = pair?;
                match key {
                    Value::String(_) | Value::Integer(_) => (),
                    Value::Number(n) if n.is_finite() && n.fract() == 0.0 => (),
                    _ => {
                        return Err(mlua::Error::runtime(
                            "authoring data key must be string or integer",
                        ));
                    }
                }
                validate_data(lua, &value, depth + 1, limit)?;
            }
            Ok(())
        }
        _ => Err(mlua::Error::runtime(
            "authoring data must be finite plain data; no functions, stacks or handles",
        )),
    }
}

fn json_bytes(value: &serde_json::Value, depth: usize, limit: usize) -> mlua::Result<usize> {
    if depth > limit {
        return Err(mlua::Error::runtime("authoring data depth exceeded"));
    }
    let mut bytes = 64usize;
    match value {
        serde_json::Value::String(value) => bytes = bytes.saturating_add(value.len()),
        serde_json::Value::Array(values) => {
            for value in values {
                bytes = bytes.saturating_add(json_bytes(value, depth + 1, limit)?);
            }
        }
        serde_json::Value::Object(values) => {
            for (key, value) in values {
                bytes = bytes.saturating_add(key.len()).saturating_add(json_bytes(
                    value,
                    depth + 1,
                    limit,
                )?);
            }
        }
        _ => (),
    }
    Ok(bytes)
}

/// Single-owner VM. Input resumes only this story coroutine. Presentation is
/// evaluated by `advance_presentation`, independently of its waiting state.
pub struct LuauAuthoringRuntime {
    vm: SandboxedLuau,
    context: Context,
    thread: Option<Thread>,
    status: StoryStatus,
}

impl LuauAuthoringRuntime {
    pub fn new(
        limits: PolicyExecutionBudget,
        scope: TaskScope,
        host: Rc<dyn AuthoringHost>,
    ) -> mlua::Result<Self> {
        let vm = SandboxedLuau::new(limits).map_err(mlua::Error::external)?;
        let context = Context {
            shared: Rc::new(RefCell::new(Shared {
                root: scope.clone(),
                scope: scope.child(),
                paragraph: None,
                story_thread: None,
                visits: BTreeMap::new(),
                records: Vec::new(),
                recoverable: RecoverableState::default(),
                segments: BTreeMap::new(),
                active_segments: BTreeSet::new(),
                cancelling: false,
                pending: None,
                budget: HostBudget::new(
                    &VnPolicyState::default(),
                    &PolicyQueryContext::default(),
                    limits,
                )?,
                limits,
            })),
            modules: vm.lua().create_table()?,
            host,
        };
        api::install(vm.lua(), &context)?;
        Ok(Self {
            vm,
            context,
            thread: None,
            status: StoryStatus::Idle,
        })
    }

    /// Evaluate a named source chunk, then atomically replace its export table.
    /// Existing coroutine frames retain their function; subsequent `rk.call`
    /// and timeline callbacks resolve against the replacement table.
    pub fn replace_module(
        &mut self,
        module: &str,
        source_name: &str,
        source: &str,
    ) -> mlua::Result<()> {
        self.context.available()?;
        if module.is_empty() {
            return Err(mlua::Error::runtime("module identity must be nonempty"));
        }
        self.begin_dispatch();
        self.context
            .charge(module.len().saturating_add(source.len()))?;
        let lua = self.vm.lua();
        let environment = lua.create_table()?;
        let metatable = lua.create_table()?;
        metatable.set("__index", lua.globals())?;
        metatable.set_readonly(true);
        environment.set_metatable(Some(metatable))?;
        environment.set_readonly(true);
        let exports: Table = lua
            .load(source)
            .set_name(source_name)
            .set_environment(environment)
            .eval()?;
        self.context.shared.borrow().budget.borrow().check()?;
        self.context.modules.set(module, exports)
    }

    pub fn start_paragraph(&mut self, paragraph: ParagraphId) -> mlua::Result<StoryStatus> {
        if self.context.shared.borrow().root.is_cancelled() {
            self.cancel()?;
            return Ok(self.status.clone());
        }
        // Resolve before interrupting a live paragraph; missing entries leave it intact.
        let module: Table = self.context.modules.get(paragraph.module.as_str())?;
        let entries: Table = module.get("paragraphs")?;
        let entry: Value = entries.get(paragraph.entry.as_str())?;
        let function = match entry {
            Value::Function(function) => function,
            Value::Table(table) => table.get("run")?,
            _ => {
                return Err(mlua::Error::runtime(
                    "paragraph entry must be a function or { run = function }",
                ));
            }
        };
        let thread = self.vm.lua().create_thread(function)?;
        self.cancel()?;
        {
            let mut shared = self.context.shared.borrow_mut();
            shared.scope = shared.root.child();
            shared.paragraph = Some(paragraph);
            shared.story_thread = Some(thread.to_pointer() as usize);
            shared.visits.clear();
            shared.records.clear();
        }
        self.thread = Some(thread);
        self.status = StoryStatus::Idle;
        self.resume(Value::Nil)
    }

    fn resume(&mut self, input: Value) -> mlua::Result<StoryStatus> {
        self.context.active()?;
        self.begin_dispatch();
        let result = self
            .thread
            .as_ref()
            .ok_or_else(|| mlua::Error::runtime("no active paragraph"))?
            .resume::<Value>(input);
        let healthy = self.context.shared.borrow().budget.borrow().check();
        if let Err(error) = healthy.and(result) {
            self.fail();
            return Err(error);
        }
        let status = self.thread.as_ref().map(Thread::status);
        if status == Some(ThreadStatus::Resumable) {
            let pending = self.context.shared.borrow().pending.clone();
            match pending {
                Some(wait) => self.status = StoryStatus::Waiting(wait),
                None => {
                    self.fail();
                    return Err(mlua::Error::runtime("story yielded outside rk.wait_input"));
                }
            }
        } else {
            self.thread = None;
            self.context.shared.borrow_mut().story_thread = None;
            self.status = StoryStatus::Completed;
        }
        Ok(self.status.clone())
    }

    /// False means a stale token or unrelated input; the coroutine is untouched.
    pub fn input(
        &mut self,
        token: u64,
        action: &str,
        payload: serde_json::Value,
    ) -> mlua::Result<bool> {
        if self.context.shared.borrow().scope.is_cancelled() {
            self.cancel()?;
            return Ok(false);
        }
        let matches = matches!(&self.status, StoryStatus::Waiting(wait) if wait.token == token && wait.action == action);
        if !matches {
            return Ok(false);
        }
        self.begin_dispatch();
        let bytes = json_bytes(
            &payload,
            0,
            self.context.shared.borrow().limits.snapshot_depth,
        )?;
        self.context.charge(bytes)?;
        let input = self.vm.lua().to_value(&payload)?;
        validate_data(
            self.vm.lua(),
            &input,
            0,
            self.context.shared.borrow().limits.snapshot_depth,
        )?;
        self.context.shared.borrow_mut().pending = None;
        self.resume(input)?;
        Ok(true)
    }

    pub fn status(&self) -> StoryStatus {
        self.status.clone()
    }

    pub fn records(&self) -> Vec<NodeRecord> {
        self.context.shared.borrow().records.clone()
    }

    pub fn recoverable_state(&self) -> RecoverableState {
        self.context.shared.borrow().recoverable.clone()
    }

    /// Installs explicit registered state while no coroutine is live. A caller
    /// restarts the paragraph from its saved entry, never from a serialized stack.
    pub fn restore_registered_state(&mut self, candidate: RecoverableState) -> mlua::Result<()> {
        if self.thread.is_some()
            || self
                .context
                .shared
                .borrow()
                .segments
                .values()
                .any(|s| s.status == SegmentStatus::Running)
        {
            return Err(mlua::Error::runtime(
                "cancel live work before installing recoverable state",
            ));
        }
        self.begin_dispatch();
        let depth = self.context.shared.borrow().limits.snapshot_depth;
        let mut bytes = 256usize;
        for values in [
            &candidate.variables,
            &candidate.scene,
            &candidate.extensions,
        ] {
            for (key, value) in values {
                bytes = bytes
                    .saturating_add(key.len())
                    .saturating_add(json_bytes(value, 0, depth)?);
            }
        }
        self.context.charge(bytes)?;
        let value = self.vm.lua().to_value(&candidate)?;
        let candidate = self.context.data(self.vm.lua(), value)?;
        self.context.shared.borrow_mut().recoverable = candidate;
        Ok(())
    }

    pub fn advance_presentation(&mut self, seconds: f64) -> mlua::Result<()> {
        if !seconds.is_finite() || seconds < 0.0 {
            return Err(mlua::Error::runtime(
                "presentation delta must be finite and nonnegative",
            ));
        }
        if self.context.shared.borrow().scope.is_cancelled() {
            return self.cancel();
        }
        self.begin_dispatch();
        // Bound this tick to work present at entry. Callbacks may start new
        // segments, which already receive their time-zero sample at creation.
        let last = self
            .context
            .shared
            .borrow()
            .active_segments
            .last()
            .copied()
            .unwrap_or(0);
        let mut previous = 0;
        loop {
            let next = self
                .context
                .shared
                .borrow()
                .active_segments
                .range((Excluded(previous), Included(last)))
                .next()
                .copied();
            let Some(id) = next else {
                break;
            };
            previous = id;
            if let Err(error) = timeline::advance(&self.context, id, seconds) {
                self.fail();
                return Err(error);
            }
        }
        Ok(())
    }

    pub fn complete_segment(&mut self, id: u64) -> mlua::Result<bool> {
        self.begin_dispatch();
        timeline::finish(&self.context, id, SegmentStatus::Completed)
    }

    pub fn cancel_segment(&mut self, id: u64) -> mlua::Result<bool> {
        self.begin_dispatch();
        timeline::finish(&self.context, id, SegmentStatus::Cancelled)
    }

    pub fn segment_status(&self, id: u64) -> Option<SegmentStatus> {
        self.context
            .shared
            .borrow()
            .segments
            .get(&id)
            .map(|segment| segment.status)
    }

    fn begin_dispatch(&self) {
        self.vm.begin_dispatch();
        self.context
            .shared
            .borrow()
            .budget
            .borrow_mut()
            .begin_dispatch();
    }

    pub fn cancel(&mut self) -> mlua::Result<()> {
        self.begin_dispatch();
        self.thread = None;
        self.context.shared.borrow_mut().pending = None;
        self.context.shared.borrow_mut().story_thread = None;
        self.context.shared.borrow_mut().cancelling = true;
        let mut first_error = None;
        loop {
            let next = self
                .context
                .shared
                .borrow()
                .active_segments
                .first()
                .copied();
            let Some(id) = next else {
                break;
            };
            if let Err(error) = timeline::finish(&self.context, id, SegmentStatus::Cancelled) {
                first_error.get_or_insert(error);
            }
        }
        self.context.shared.borrow().scope.cancel();
        self.context.shared.borrow_mut().cancelling = false;
        self.status = StoryStatus::Cancelled;
        match first_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn fail(&mut self) {
        if let Err(error) = self.cancel() {
            tracing::error!(%error, "authoring failure cleanup failed");
        }
        self.status = StoryStatus::Failed;
    }
}

impl Drop for LuauAuthoringRuntime {
    fn drop(&mut self) {
        if let Err(error) = self.cancel() {
            tracing::error!(%error, "authoring cancellation cleanup failed");
        }
    }
}
