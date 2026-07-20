import { expect, test } from "vitest";
import { overlayViewport } from "./overlayViewport";

test("no scroll when rows fit", () => {
  expect(overlayViewport(5, 2, 0)).toEqual({ offset: 0, dotsAbove: false, dotsBelow: false });
});

test("clamps offset and follows selection down", () => {
  expect(overlayViewport(10, 9, 0)).toEqual({ offset: 3, dotsAbove: true, dotsBelow: false });
});

test("follows selection up", () => {
  expect(overlayViewport(10, 1, 3)).toEqual({ offset: 1, dotsAbove: true, dotsBelow: true });
});
