import { expect, test } from "vitest";
import { formatClipAge, overlayViewport } from "./overlayViewport";

test("formatClipAge buckets", () => {
  expect(formatClipAge(0, "01:00 PM")).toBe("now");
  expect(formatClipAge(1, "01:00 PM")).toBe("1m ago");
  expect(formatClipAge(59, "01:00 PM")).toBe("59m ago");
  expect(formatClipAge(60, "01:00 PM")).toBe("1h 00m");
  expect(formatClipAge(125, "01:00 PM")).toBe("2h 05m");
});

test("formatClipAge falls back to the clock string when age is unknown", () => {
  expect(formatClipAge(null, "01:00 PM")).toBe("01:00 PM");
  expect(formatClipAge(-5, "01:00 PM")).toBe("01:00 PM");
});

test("no scroll when rows fit", () => {
  expect(overlayViewport(5, 2, 0)).toEqual({ offset: 0, dotsAbove: false, dotsBelow: false });
});

test("clamps offset and follows selection down", () => {
  expect(overlayViewport(10, 9, 0)).toEqual({ offset: 3, dotsAbove: true, dotsBelow: false });
});

test("follows selection up", () => {
  expect(overlayViewport(10, 1, 3)).toEqual({ offset: 1, dotsAbove: true, dotsBelow: true });
});
