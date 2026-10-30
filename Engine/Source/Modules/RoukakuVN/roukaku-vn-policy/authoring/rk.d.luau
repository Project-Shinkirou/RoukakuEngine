type NodeOccurrence = {
    paragraph: { module: string, entry: string },
    node: string,
    visit: number,
}

type ChoiceOption = { id: string, text: string }
type StageActionData = { [string]: any }
type TimelineKeyframe = { time: number, value: number }
type TimelineTrack = { target: string, property: string, keys: { TimelineKeyframe } }
type TimelineCallback = {
    time: number,
    call: { module: string, name: string },
    complete: { module: string, name: string },
}
type TimelineDeclaration = {
    duration: number,
    tracks: { TimelineTrack },
    callbacks: { TimelineCallback }?,
    on_complete: { module: string, name: string }?,
    on_cancel: { module: string, name: string }?,
}
type TextLayoutRequest = { [string]: any }
type TextMeasurement = {
    width: number,
    height: number,
    clipped: boolean,
    ellipsized: boolean,
    revision: string,
}
type ParagraphEntry = () -> ()
type ParagraphModule = { paragraphs: { [string]: ParagraphEntry } }

type RkApi = {
    call: (module: string, export_name: string, ...any) -> ...any,
    enter: (module: string, entry: string) -> (),
    wait_input: (action: string) -> any,
    choice: (node: string, options: { ChoiceOption }, data: any?) -> NodeOccurrence,
    on_fence: (fence: string, module: string, export_name: string, args: any) -> (),
    present: (node: string, action: string, data: StageActionData) -> (),
    line: (node: string, data: { [string]: any }) -> NodeOccurrence,
    choice_result: (occurrence: NodeOccurrence, selected: string) -> (),
    random_u64: () -> number,
    host_query: (operation: string, args: any) -> any,
    once: (id: string) -> boolean,
    register_state: (scope: string, key: string, initial: any) -> any,
    get_state: (scope: string, key: string) -> any,
    set_state: (scope: string, key: string, value: any) -> (),
    measure_text: (request: TextLayoutRequest) -> TextMeasurement,
    display_text: (id: string, request: TextLayoutRequest) -> TextMeasurement,
    set_property: (target: string, property: string, value: number) -> (),
    segment: (declaration: TimelineDeclaration) -> number,
    complete: (id: number) -> boolean,
    cancel: (id: number) -> boolean,
}

local rk: RkApi = nil :: any
