// SPDX-License-Identifier: AGPL-3.0-or-later
import { describe, expect, it } from "vitest";
import { SessionView } from "@/components/SessionView";
import { baseState, newSessionHtml, render, viewProps } from "./view-fixtures";

describe("collect viewer names switch", () => {
  it("offers an off-by-default switch to collect viewer names", () => {
    const off = newSessionHtml(false);
    expect(off).toContain('id="collect-names"');
    expect(off).toContain("Collect viewer names");
    expect(off).toContain('aria-checked="false"');
    expect(off).toContain("Viewers stay anonymous");
    const on = newSessionHtml(false, true);
    expect(on).toContain('aria-checked="true"');
    expect(on).toContain("the log shows who changed each table");
  });
});

describe("viewer names in the session view", () => {
  it("asks for a name in a modal prompt and makes the page inert", () => {
    const state = baseState({ isCreator: false, collectNames: true });
    const closed = render(<SessionView {...viewProps(state)} />);
    expect(closed).not.toContain('id="viewer-name"');
    expect(closed).not.toContain("inert");
    const html = render(
      <SessionView
        {...viewProps(state, {
          namePromptOpen: true,
          nameDraft: "Bokwa",
          nameError: "Someone in this session already uses that name. Choose another one.",
        })}
      />,
    );
    expect(html).toContain('<main class="page" inert=""');
    expect(html).toContain('<dialog open="" class="name-prompt"');
    expect(html).toContain('aria-modal="true"');
    expect(html).toContain("Your name");
    expect(html).toContain('id="viewer-name"');
    expect(html).toContain('value="Bokwa"');
    expect(html).toContain('maxLength="40"');
    expect(html).toContain('role="alert"');
    expect(html).toContain("already uses that name");
    expect(html).toContain("Continue");
    expect(html).not.toContain("Cancel");
    const changing = render(
      <SessionView {...viewProps(state, { namePromptOpen: true, nameCanCancel: true })} />,
    );
    expect(changing).toContain("Cancel");
  });

  it("shows a named viewer its name with a way to change it", () => {
    const html = render(
      <SessionView
        {...viewProps(baseState({ isCreator: false, collectNames: true }), {
          viewerName: "Alice",
        })}
      />,
    );
    expect(html).toContain("Your name: <strong>Alice</strong>");
    expect(html).toContain("Change name");
    const anonymous = render(
      <SessionView {...viewProps(baseState({ isCreator: false }), { viewerName: "Alice" })} />,
    );
    expect(anonymous).not.toContain("Change name");
  });

  it("tells the creator that names are collected", () => {
    const html = render(
      <SessionView {...viewProps(baseState({ isCreator: true, collectNames: true }))} />,
    );
    expect(html).toContain("Viewer names are collected");
    expect(html).toContain("&quot;host&quot;");
    const plain = render(<SessionView {...viewProps(baseState({ isCreator: true }))} />);
    expect(plain).not.toContain("Viewer names are collected");
  });
});
