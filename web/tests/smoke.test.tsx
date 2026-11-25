// SPDX-License-Identifier: AGPL-3.0-or-later
import { renderToStaticMarkup } from "react-dom/server";
import { MemoryRouter } from "react-router";
import { describe, expect, it } from "vitest";
import { AppRoutes } from "@/App";

describe("AppRoutes", () => {
  it("renders the lobby title at the root route", () => {
    const html = renderToStaticMarkup(
      <MemoryRouter initialEntries={["/"]}>
        <AppRoutes />
      </MemoryRouter>,
    );
    expect(html).toContain("<h1");
    expect(html).toContain("Classroom Traffic Lights");
  });
});
