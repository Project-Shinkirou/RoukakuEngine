import init from "./roukaku_player_web.js";
const launch = document.createElement("button");
launch.type = "button";
launch.textContent = "Start";
launch.dataset.roukakuPermissionHandshake = "pending";
document.body.appendChild(launch);

launch.addEventListener("click", async () => {
  launch.disabled = true;
  try {
    await init("./roukaku_player_web_bg.wasm");
    launch.dataset.roukakuPermissionHandshake = "complete";
    launch.remove();
  } catch (error) {
    launch.disabled = false;
    launch.dataset.roukakuPermissionHandshake = "blocked";
    throw error;
  }
}, { once: false });
