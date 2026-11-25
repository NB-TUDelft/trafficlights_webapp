// SPDX-License-Identifier: AGPL-3.0-or-later

import { Plus } from "lucide-react";
import { useEffect, useMemo, useReducer, useState } from "react";
import { Link } from "react-router";
import { Footer } from "@/components/Footer";
import { SessionCard } from "@/components/SessionCard";
import { ReconnectingSocket, type SocketStatus, websocketUrl } from "@/lib/socket";
import { parseLobbyServerMsg } from "@/protocol/parse";
import { initialLobbyState, type LobbyState, lobbyReducer } from "@/protocol/reducers";
import { loadCreatorTokens } from "@/protocol/storage";
import { EXPECTED_PROTO_VERSION } from "@/protocol/types";

const LOGO_SIZE = 78;
const ACTION_ICON_SIZE = 18;

function statusLabel(status: SocketStatus): string {
  if (status === "open") {
    return "Live";
  }
  if (status === "reconnecting") {
    return "Reconnecting...";
  }
  if (status === "closed") {
    return "Disconnected";
  }
  return "Connecting...";
}

function statusKind(status: SocketStatus): string {
  if (status === "open") {
    return "live";
  }
  if (status === "closed") {
    return "disconnected";
  }
  return "reconnecting";
}

export interface HomeViewProps {
  state: LobbyState;
  status: SocketStatus;
  ownSessionIds: ReadonlySet<string>;
}

export function HomeView({ state, status, ownSessionIds }: Readonly<HomeViewProps>) {
  return (
    <main className="page">
      <header className="lobby-header">
        <div className="lobby-brand">
          <span className="brand-mark brand-mark-lg">
            <img src="/icon.svg" width={LOGO_SIZE} height={LOGO_SIZE} alt="" />
          </span>
          <div className="lobby-brand-copy">
            <h1>Classroom Traffic Lights</h1>
            <p>
              Teachers stream a micro:bit USB receiver from their browser and share live table
              status with TAs and co-teachers, so questions and requests for help are visible at a
              glance.
            </p>
          </div>
        </div>
        <span className="status-chip" data-status={statusKind(status)} aria-live="polite">
          {statusLabel(status)}
        </span>
      </header>
      {state.error === null ? null : (
        <div className="banner banner-error" role="alert">
          {state.error.message}
        </div>
      )}
      {state.serverInfo !== null && state.serverInfo.proto_version !== EXPECTED_PROTO_VERSION ? (
        <div className="banner banner-warning" role="alert">
          This page is out of date for the server. Please reload the page.
        </div>
      ) : null}
      <section className="sessions-section" aria-label="Active sessions">
        <div className="sessions-toolbar">
          <div className="sessions-heading">
            <h2>Active sessions</h2>
            {state.sessions.length > 0 ? (
              <span className="count-pill">{state.sessions.length}</span>
            ) : null}
          </div>
          <Link className="button-link add-session-btn" to="/new">
            <Plus size={ACTION_ICON_SIZE} aria-hidden="true" />
            <span>Add session</span>
          </Link>
        </div>
        {state.sessions.length === 0 ? (
          <div className="sessions-empty">
            <p className="muted-text">No active sessions yet. Add a session to start streaming.</p>
          </div>
        ) : (
          <div className="session-list">
            {state.sessions.map((session) => (
              <SessionCard
                key={session.session_id}
                session={session}
                isOwn={ownSessionIds.has(session.session_id)}
              />
            ))}
          </div>
        )}
      </section>
      <Footer serverInfo={state.serverInfo} />
    </main>
  );
}

export function HomePage() {
  const [state, dispatch] = useReducer(lobbyReducer, initialLobbyState);
  const [status, setStatus] = useState<SocketStatus>("connecting");
  const ownSessionIds = useMemo(() => new Set(Object.keys(loadCreatorTokens())), []);
  useEffect(() => {
    const socket = new ReconnectingSocket(
      websocketUrl("/ws/lobby"),
      {
        onMessage: (data) => {
          const msg = parseLobbyServerMsg(data);
          if (msg !== null) {
            dispatch(msg);
          }
        },
        onStatusChange: setStatus,
      },
      { pingMessage: () => JSON.stringify({ type: "ping" }) },
    );
    socket.connect();
    return () => socket.close();
  }, []);
  return <HomeView state={state} status={status} ownSessionIds={ownSessionIds} />;
}
