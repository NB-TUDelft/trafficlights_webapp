// SPDX-License-Identifier: AGPL-3.0-or-later

import { useEffect, useMemo, useReducer, useState } from "react";
import { Link } from "react-router";
import { Footer } from "@/components/Footer";
import { SessionCard } from "@/components/SessionCard";
import { ReconnectingSocket, type SocketStatus, websocketUrl } from "@/lib/socket";
import { parseLobbyServerMsg } from "@/protocol/parse";
import { initialLobbyState, type LobbyState, lobbyReducer } from "@/protocol/reducers";
import { loadCreatorTokens } from "@/protocol/storage";
import { EXPECTED_PROTO_VERSION } from "@/protocol/types";

const ICON_SIZE = 64;

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

export interface HomeViewProps {
  state: LobbyState;
  status: SocketStatus;
  ownSessionIds: ReadonlySet<string>;
}

export function HomeView({ state, status, ownSessionIds }: HomeViewProps) {
  return (
    <main className="page">
      <header className="hero">
        <img src="/icon.svg" width={ICON_SIZE} height={ICON_SIZE} alt="" />
        <h1>Classroom Traffic Lights</h1>
        <p>
          Teachers stream a micro:bit USB receiver from their browser and share live table status
          with TAs and co-teachers, so questions and requests for help are visible at a glance.
        </p>
        <p className="hero-actions">
          <Link className="button-link" to="/new">
            Add session
          </Link>
          <span className="status-chip">{statusLabel(status)}</span>
        </p>
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
      <section aria-label="Active sessions">
        <h2>Active sessions</h2>
        {state.sessions.length === 0 ? (
          <p className="muted-text">No active sessions yet. Add a session to start streaming.</p>
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
