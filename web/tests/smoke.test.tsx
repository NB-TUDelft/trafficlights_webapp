// SPDX-License-Identifier: AGPL-3.0-or-later
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { App } from "@/App";

describe("App", () => {
  it("renders the application title", () => {
    const html = renderToStaticMarkup(<App />);
    expect(html).toContain("<h1");
    expect(html).toContain("Classroom Traffic Lights");
  });
});
