import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import "./styles.css";

type LaunchOptions = {
  count: number;
  appPath: string | null;
  minDelayMs: number;
  maxDelayMs: number;
  preferRegistry: boolean;
  macosCloneInstances: boolean;
  windowsSafeMode: boolean;
  safeModePassword: string | null;
  safeModeUserPrefix: string;
};

type LaunchResult = {
  success: boolean;
  pid: number | null;
  message: string;
  index: number;
};

type BatchResult = {
  results: LaunchResult[];
  platform: string;
};

type AppInfo = {
  platform: string;
  resolvedPath: string | null;
  runningCount: number;
  runningPids: number[];
};

type SafeModeUserStatus = {
  index: number;
  username: string;
  exists: boolean;
};

type Slot = {
  index: number;
  alias: string;
  username: string;
  userExists: boolean;
  status: "idle" | "running" | "error";
  pid: number | null;
  message: string;
};

const MAX = 10;
const slots: Slot[] = Array.from({ length: MAX }, (_, i) => ({
  index: i + 1,
  alias: `账号 ${i + 1}`,
  username: `WeComSlot${i + 1}`,
  userExists: false,
  status: "idle",
  pid: null,
  message: "",
}));

const els = {
  platformBadge: document.getElementById("platformBadge")!,
  statusText: document.getElementById("statusText")!,
  appPath: document.getElementById("appPath") as HTMLInputElement,
  batchCount: document.getElementById("batchCount") as HTMLInputElement,
  minDelay: document.getElementById("minDelay") as HTMLInputElement,
  maxDelay: document.getElementById("maxDelay") as HTMLInputElement,
  preferRegistry: document.getElementById("preferRegistry") as HTMLInputElement,
  windowsSafeMode: document.getElementById("windowsSafeMode") as HTMLInputElement,
  safePrefix: document.getElementById("safePrefix") as HTMLInputElement,
  safePassword: document.getElementById("safePassword") as HTMLInputElement,
  macosClone: document.getElementById("macosClone") as HTMLInputElement,
  winOpt: document.getElementById("winOpt")!,
  winSafeOpt: document.getElementById("winSafeOpt")!,
  safeModePanel: document.getElementById("safeModePanel")!,
  macOpt: document.getElementById("macOpt")!,
  runningCount: document.getElementById("runningCount")!,
  slotsBody: document.getElementById("slotsBody")!,
  logs: document.getElementById("logs")!,
  btnDetect: document.getElementById("btnDetect") as HTMLButtonElement,
  btnBrowse: document.getElementById("btnBrowse") as HTMLButtonElement,
  btnOne: document.getElementById("btnOne") as HTMLButtonElement,
  btnBatch: document.getElementById("btnBatch") as HTMLButtonElement,
  btnRefresh: document.getElementById("btnRefresh") as HTMLButtonElement,
  btnKill: document.getElementById("btnKill") as HTMLButtonElement,
  btnPrepareUsers: document.getElementById("btnPrepareUsers") as HTMLButtonElement,
  btnRefreshUsers: document.getElementById("btnRefreshUsers") as HTMLButtonElement,
};

let busy = false;
let platform = "unknown";

function log(msg: string) {
  const li = document.createElement("li");
  const t = new Date().toLocaleTimeString();
  li.textContent = `[${t}] ${msg}`;
  els.logs.prepend(li);
  while (els.logs.children.length > 200) {
    els.logs.lastChild?.remove();
  }
}

function setBusy(v: boolean) {
  busy = v;
  [
    els.btnOne,
    els.btnBatch,
    els.btnDetect,
    els.btnBrowse,
    els.btnKill,
    els.btnPrepareUsers,
    els.btnRefreshUsers,
  ].forEach((b) => (b.disabled = v));
}

function syncSafePanel() {
  const on =
    platform === "windows" && els.windowsSafeMode.checked;
  els.safeModePanel.style.display = on ? "" : "none";
}

function renderSlots() {
  const prefix = els.safePrefix.value.trim() || "WeComSlot";
  els.slotsBody.innerHTML = "";
  for (const s of slots) {
    s.username = `${prefix}${s.index}`;
    const userBadge = s.userExists
      ? `<span class="badge running">${escapeHtml(s.username)}</span>`
      : `<span class="badge idle">${escapeHtml(s.username)} · 未创建</span>`;
    const tr = document.createElement("tr");
    tr.innerHTML = `
      <td>${s.index}</td>
      <td><input data-alias="${s.index}" value="${escapeHtml(s.alias)}" /></td>
      <td>${userBadge}</td>
      <td><span class="badge ${s.status}">${s.status}</span></td>
      <td>${s.pid ?? "—"}</td>
      <td>${escapeHtml(s.message)}</td>
    `;
    els.slotsBody.appendChild(tr);
  }
  els.slotsBody.querySelectorAll("input[data-alias]").forEach((input) => {
    input.addEventListener("change", (e) => {
      const el = e.target as HTMLInputElement;
      const idx = Number(el.dataset.alias);
      const slot = slots.find((x) => x.index === idx);
      if (slot) slot.alias = el.value;
    });
  });
}

function escapeHtml(s: string) {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function options(count: number): LaunchOptions {
  return {
    count,
    appPath: els.appPath.value.trim() || null,
    minDelayMs: Number(els.minDelay.value) || 2500,
    maxDelayMs: Number(els.maxDelay.value) || 6000,
    preferRegistry: els.preferRegistry.checked,
    macosCloneInstances: els.macosClone.checked,
    windowsSafeMode: els.windowsSafeMode.checked,
    safeModePassword: els.safePassword.value || null,
    safeModeUserPrefix: els.safePrefix.value.trim() || "WeComSlot",
  };
}

async function refreshUsers() {
  if (platform !== "windows") return;
  const count = Math.min(MAX, Math.max(1, Number(els.batchCount.value) || 8));
  const list = await invoke<SafeModeUserStatus[]>("list_safe_mode_users", {
    count,
    userPrefix: els.safePrefix.value.trim() || "WeComSlot",
  });
  list.forEach((u) => {
    const slot = slots[u.index - 1];
    if (!slot) return;
    slot.username = u.username;
    slot.userExists = u.exists;
  });
  renderSlots();
}

async function refreshInfo() {
  const info = await invoke<AppInfo>("get_app_info", {
    appPath: els.appPath.value.trim() || null,
  });
  platform = info.platform;
  els.platformBadge.textContent = info.platform;
  els.runningCount.textContent = String(info.runningCount);
  if (!els.appPath.value && info.resolvedPath) {
    els.appPath.value = info.resolvedPath;
  }
  const isWin = info.platform === "windows";
  els.winOpt.style.display = isWin ? "" : "none";
  els.winSafeOpt.style.display = isWin ? "" : "none";
  els.macOpt.style.display = info.platform === "macos" ? "" : "none";
  syncSafePanel();
  els.statusText.textContent = `运行中企业微信进程: ${info.runningCount}`;
  if (isWin) await refreshUsers();
  else renderSlots();
}

async function launchOne() {
  if (busy) return;
  setBusy(true);
  els.statusText.textContent = "正在启动…";
  try {
    const result = await invoke<LaunchResult>("launch_one", {
      options: options(1),
    });
    const slot =
      slots.find((s) => s.status !== "running") ?? slots[0];
    slot.status = result.success ? "running" : "error";
    slot.pid = result.pid;
    slot.message = result.message;
    log(result.message);
    els.statusText.textContent = result.message;
    renderSlots();
    await refreshInfo();
  } catch (e) {
    log(String(e));
    els.statusText.textContent = String(e);
  } finally {
    setBusy(false);
  }
}

async function launchBatch() {
  if (busy) return;
  const count = Math.min(MAX, Math.max(1, Number(els.batchCount.value) || 8));
  setBusy(true);
  els.statusText.textContent = `分批启动 ${count} 个实例…`;
  for (let i = 0; i < count; i++) {
    slots[i].status = "idle";
    slots[i].pid = null;
    slots[i].message = "";
  }
  renderSlots();
  try {
    const batch = await invoke<BatchResult>("launch_batch", {
      options: options(count),
    });
    batch.results.forEach((r) => {
      const slot = slots[r.index - 1];
      if (!slot) return;
      slot.status = r.success ? "running" : "error";
      slot.pid = r.pid;
      slot.message = r.message;
      log(r.message);
    });
    const ok = batch.results.filter((r) => r.success).length;
    els.statusText.textContent = `批量完成：成功 ${ok} / 尝试 ${batch.results.length}（${batch.platform}）`;
    renderSlots();
    await refreshInfo();
  } catch (e) {
    log(String(e));
    els.statusText.textContent = String(e);
  } finally {
    setBusy(false);
  }
}

els.btnDetect.addEventListener("click", async () => {
  const path = await invoke<string | null>("resolve_path", { appPath: null });
  if (path) {
    els.appPath.value = path;
    log(`已定位: ${path}`);
  } else {
    log("未检测到企业微信");
  }
  await refreshInfo();
});

els.btnBrowse.addEventListener("click", async () => {
  const selected = await open({
    multiple: false,
    directory: platform === "macos",
    filters:
      platform === "windows"
        ? [{ name: "WXWork", extensions: ["exe"] }]
        : undefined,
  });
  if (typeof selected === "string") {
    els.appPath.value = selected;
    log(`已选择: ${selected}`);
  }
});

els.windowsSafeMode.addEventListener("change", () => {
  syncSafePanel();
  void refreshUsers();
});
els.safePrefix.addEventListener("change", () => void refreshUsers());
els.batchCount.addEventListener("change", () => void refreshUsers());

els.btnPrepareUsers.addEventListener("click", async () => {
  const password = els.safePassword.value.trim();
  if (password.length < 8) {
    log("密码至少 8 位");
    return;
  }
  setBusy(true);
  try {
    const result = await invoke<{
      created: string[];
      alreadyExisted: string[];
      failed: string[];
      message: string;
    }>("prepare_safe_mode_users", {
      req: {
        count: Number(els.batchCount.value) || 8,
        password,
        userPrefix: els.safePrefix.value.trim() || "WeComSlot",
      },
    });
    log(result.message);
    if (result.failed?.length) {
      result.failed.forEach((f) => log(`失败: ${f}`));
    }
    els.statusText.textContent = result.message;
    await refreshUsers();
  } catch (e) {
    log(String(e));
    els.statusText.textContent = String(e);
  } finally {
    setBusy(false);
  }
});

els.btnRefreshUsers.addEventListener("click", () => void refreshUsers());
els.btnOne.addEventListener("click", () => void launchOne());
els.btnBatch.addEventListener("click", () => void launchBatch());
els.btnRefresh.addEventListener("click", () => void refreshInfo());
els.btnKill.addEventListener("click", async () => {
  const n = await invoke<number>("kill_all");
  slots.forEach((s) => {
    s.status = "idle";
    s.pid = null;
    s.message = "";
  });
  renderSlots();
  log(`已结束 ${n} 个进程`);
  await refreshInfo();
});

renderSlots();
void refreshInfo().catch((e) => log(String(e)));
