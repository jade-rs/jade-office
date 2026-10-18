// The browser side of the prototype: fetch the wasm module and the fonts,
// open a .pptx or .xlsx, and put the pixels the Rust painter returns on
// canvases. Everything about the document — reading, layout, painting — is
// the engine in wasm; this file only moves bytes and pixels.
import init, { WebFonts, WebDeck, WebWorkbook, WebDocument } from "./pkg/jade_web.js";

const status = document.getElementById("status");
const stage = document.getElementById("stage");
const thumbs = document.getElementById("thumbs");
const tabs = document.getElementById("tabs");

function draw(canvas, rendered) {
  const { width, height } = rendered;
  canvas.width = width;
  canvas.height = height;
  const pixels = new Uint8ClampedArray(rendered.takeRgba());
  canvas.getContext("2d").putImageData(new ImageData(pixels, width, height), 0, 0);
  rendered.free();
}

let fonts;
let cleanup = () => {};

async function boot() {
  const started = performance.now();
  await init();
  const [regular, bold] = await Promise.all(
    ["fonts/NotoSansJP-Regular.ttf", "fonts/NotoSansJP-Bold.ttf"].map((url) =>
      fetch(url).then((response) => response.arrayBuffer()),
    ),
  );
  fonts = new WebFonts(new Uint8Array(regular), new Uint8Array(bold));
  status.textContent = `準備完了 (${Math.round(performance.now() - started)} ms)`;
}

function openDeck(bytes, name) {
  const deck = new WebDeck(bytes, fonts);
  const ratio = window.devicePixelRatio || 1;
  thumbs.style.display = "block";
  tabs.style.display = "none";
  thumbs.replaceChildren();
  stage.replaceChildren();
  const main = document.createElement("canvas");
  main.className = "slide";
  stage.append(main);

  const show = (index) => {
    const started = performance.now();
    const width = Math.min(stage.clientWidth - 48, 1100);
    draw(main, deck.renderSlide(index, Math.round(width * ratio)));
    main.style.width = `${width}px`;
    [...thumbs.children].forEach((thumb, i) => thumb.classList.toggle("active", i === index));
    status.textContent = `${name} — スライド ${index + 1} / ${deck.slideCount()} — ${(performance.now() - started).toFixed(1)} ms`;
  };
  for (let index = 0; index < deck.slideCount(); index++) {
    const thumb = document.createElement("canvas");
    draw(thumb, deck.renderSlide(index, Math.round(170 * ratio)));
    thumb.addEventListener("click", () => show(index));
    thumbs.append(thumb);
  }
  show(0);
  cleanup = () => deck.free();
}

function openDocument(bytes, name) {
  // Every page is laid out and painted when the document opens, at the
  // display's pixel density; they are shown as a column of canvases.
  const ratio = window.devicePixelRatio || 1;
  const started = performance.now();
  const document_ = new WebDocument(bytes, ratio, fonts);
  thumbs.style.display = "none";
  tabs.style.display = "none";
  stage.replaceChildren();
  for (let index = 0; index < document_.pageCount(); index++) {
    const canvas = document.createElement("canvas");
    canvas.className = "slide";
    draw(canvas, document_.renderPage(index));
    canvas.style.width = `${canvas.width / ratio}px`;
    stage.append(canvas);
  }
  status.textContent = `${name} — ${document_.pageCount()} ページ — ${Math.round(performance.now() - started)} ms`;
  cleanup = () => document_.free();
}

function openWorkbook(bytes, name) {
  const book = new WebWorkbook(bytes, fonts);
  const ratio = window.devicePixelRatio || 1;
  thumbs.style.display = "none";
  stage.replaceChildren();
  const spacer = document.createElement("div");
  spacer.id = "sheet-spacer";
  const canvas = document.createElement("canvas");
  canvas.id = "sheet";
  stage.append(spacer, canvas);

  // The sheet is painted a viewport at a time, like the native view: the
  // spacer gives the scroll range, the canvas stays put and repaints.
  let frame = 0;
  const paint = () => {
    frame = 0;
    const started = performance.now();
    const width = stage.clientWidth;
    const height = stage.clientHeight;
    draw(canvas, book.renderViewport(stage.scrollLeft, stage.scrollTop,
      Math.round(width * ratio), Math.round(height * ratio), ratio));
    canvas.style.width = `${width}px`;
    canvas.style.height = `${height}px`;
    canvas.style.marginTop = "0";
    status.textContent = `${name} — ${(performance.now() - started).toFixed(1)} ms`;
  };
  const layout = () => {
    spacer.style.width = `${book.sheetWidth()}px`;
    spacer.style.height = `${book.sheetHeight()}px`;
    paint();
  };
  const onScroll = () => { if (!frame) frame = requestAnimationFrame(paint); };
  stage.addEventListener("scroll", onScroll);

  tabs.replaceChildren();
  tabs.style.display = "flex";
  book.sheetNames().forEach((sheet, index) => {
    const button = document.createElement("button");
    button.textContent = sheet;
    button.addEventListener("click", () => {
      book.selectSheet(index);
      [...tabs.children].forEach((tab, i) => tab.classList.toggle("active", i === index));
      stage.scrollTo(0, 0);
      layout();
    });
    tabs.append(button);
  });
  tabs.firstChild?.classList.add("active");
  layout();
  cleanup = () => { stage.removeEventListener("scroll", onScroll); book.free(); };
}

function open(bytes, name) {
  cleanup();
  try {
    // Macro-enabled and template forms open the same way as the plain ones.
    const extension = name.toLowerCase().split(".").pop();
    if (["pptx", "pptm", "potx", "potm", "ppsx", "ppsm"].includes(extension)) openDeck(bytes, name);
    else if (["docx", "docm", "dotx", "dotm"].includes(extension)) openDocument(bytes, name);
    else openWorkbook(bytes, name);
  } catch (error) {
    status.textContent = `${name}: ${error}`;
  }
}

document.getElementById("file").addEventListener("change", async (event) => {
  const file = event.target.files[0];
  if (file) open(new Uint8Array(await file.arrayBuffer()), file.name);
});
document.querySelectorAll("[data-sample]").forEach((button) =>
  button.addEventListener("click", async () => {
    const name = button.dataset.sample;
    const bytes = await fetch(`samples/${name}`).then((response) => response.arrayBuffer());
    open(new Uint8Array(bytes), name);
  }),
);

boot().catch((error) => { status.textContent = `起動に失敗: ${error}`; });
