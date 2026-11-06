import "@testing-library/jest-dom/vitest";

// jsdom has no ResizeObserver; @mui/x-charts needs one to mount.
if (!("ResizeObserver" in globalThis)) {
  globalThis.ResizeObserver = class {
    observe() {}
    unobserve() {}
    disconnect() {}
  };
}
