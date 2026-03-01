import "./index.css";
import init, { InitOutput } from "../wasm/main";
let engine: InitOutput;

async function main() {
  try {
    window.app = {};
    window.app.reroll_handlers = {};
    engine = await init();
    requestAnimationFrame(resizeCanvas);

    // this statement will only return if the Rust program exits
    engine.run();
  } catch (e) {
    const error = e as Error;
    if (error.message.includes("Using exceptions for control flow")) {
      return;
    }
    console.error("Failed to start Bevy app:", error);
  }
}

async function resizeCanvas() {
  const canvas = document.querySelector("canvas");
  if (!canvas) return;
  const dppx = window.devicePixelRatio;
  canvas.width = window.innerWidth * dppx;
  canvas.height = window.innerHeight * dppx;
  canvas.style.width = `${window.innerWidth}px`;
  canvas.style.height = `${window.innerHeight}px`;
}

window.addEventListener("resize", resizeCanvas);

main();

if (import.meta.hot) {
  import.meta.hot.on("wgsl-update", async ({ file }) => {
    console.log(`[wgsl-hmr] ${file}`);
    const encoder = new TextEncoder();
    const fileBuffer = encoder.encode(file);
    const len = fileBuffer.length;
    const alignment = 1;
    const ptr = engine.__wbindgen_malloc(len, 1);
    new Uint8Array(engine.memory.buffer).set(fileBuffer, ptr);
    engine.reload_shader(ptr, len);
    engine.__wbindgen_free(ptr, len, alignment);
  });
}
