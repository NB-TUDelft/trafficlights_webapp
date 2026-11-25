// SPDX-License-Identifier: AGPL-3.0-or-later

import { Route, Routes } from "react-router";
import { HomePage } from "@/pages/HomePage";
import { NewSessionPage } from "@/pages/NewSessionPage";
import { SessionPage } from "@/pages/SessionPage";

/** Route table without a router, so tests can wrap it in MemoryRouter. */
export function AppRoutes() {
  return (
    <Routes>
      <Route path="/" element={<HomePage />} />
      <Route path="/new" element={<NewSessionPage />} />
      <Route path="/s/:id" element={<SessionPage />} />
    </Routes>
  );
}

export function App() {
  return <AppRoutes />;
}
