// SPDX-License-Identifier: AGPL-3.0-or-later

import { ArrowLeft } from "lucide-react";
import { useCallback, useEffect, useReducer, useRef, useState } from "react";
import { Link, useNavigate } from "react-router";
import { Footer } from "@/components/Footer";
import { ToggleSwitch } from "@/components/ToggleSwitch";
import { ReconnectingSocket, websocketUrl } from "@/lib/socket";
import { parseLobbyServerMsg } from "@/protocol/parse";
import { initialLobbyState, lobbyReducer } from "@/protocol/reducers";
import { saveCreatorToken } from "@/protocol/storage";
import type { LobbyClientMsg, ServerBuildInfo } from "@/protocol/types";
import {
  DEFAULT_RANGE_END,
  DEFAULT_RANGE_START,
  isValidTableRange,
  MAX_SESSION_NAME_CHARS,
  MAX_TABLE_ID,
  MIN_TABLE_ID,
  normalizeSessionName,
} from "@/protocol/types";
import { isWebSerialSupported, requestMicrobitPort } from "@/serial/loader";
import { bindPendingSerialPort, setPendingSerialPort } from "@/serial/pending";

const PARSE_RADIX = 10;
const NEW_LOGO_SIZE = 58;
const NEW_BACK_ICON_SIZE = 16;

export interface NewSessionViewProps {
  serverInfo: ServerBuildInfo | null;
  name: string;
  startValue: string;
  endValue: string;
  pin: string;
  collectNames: boolean;
  showAllPorts: boolean;
  serialSupported: boolean;
  hasPendingPort: boolean;
  error: string | null;
  submitting: boolean;
  onNameChange: (value: string) => void;
  onStartChange: (value: string) => void;
  onEndChange: (value: string) => void;
  onPinChange: (value: string) => void;
  onCollectNamesChange: (value: boolean) => void;
  onShowAllChange: (value: boolean) => void;
  onSelectPort: () => void;
  onSubmit: () => void;
}

function SerialPicker(props: NewSessionViewProps) {
  if (!props.serialSupported) {
    return (
      <p className="banner banner-warning">
        Web Serial is not available here. Use a Chromium-based browser over HTTPS or localhost to
        stream a micro:bit receiver. You can still create the session and connect later.
      </p>
    );
  }
  return (
    <div className="field-group">
      <button type="button" onClick={props.onSelectPort}>
        Select micro:bit receiver
      </button>
      <label className="check-label">
        <input
          type="checkbox"
          checked={props.showAllPorts}
          onChange={(event) => props.onShowAllChange(event.target.checked)}
        />{" "}
        Show all USB serial ports
      </label>
      {props.hasPendingPort ? <p className="muted-text">Receiver selected and ready.</p> : null}
    </div>
  );
}

function collectNamesHint(collectNames: boolean): string {
  return collectNames
    ? 'Viewers enter a name; the log shows who changed each table (you as "host")'
    : "Viewers stay anonymous";
}

export function NewSessionView(props: NewSessionViewProps) {
  return (
    <main className="page">
      <header className="lobby-header">
        <div className="lobby-brand">
          <span className="brand-mark brand-mark-nav">
            <img src="/icon.svg" width={NEW_LOGO_SIZE} height={NEW_LOGO_SIZE} alt="" />
          </span>
          <div className="lobby-brand-copy">
            <h1>Add session</h1>
            <p>
              Name your classroom session, choose its table range, and optionally pick the receiver
              now.
            </p>
          </div>
        </div>
        <Link className="lobby-link" to="/">
          <span className="lobby-link-label">
            <ArrowLeft size={NEW_BACK_ICON_SIZE} aria-hidden="true" />
            <span>Back to Lobby</span>
          </span>
        </Link>
      </header>
      {props.error === null ? null : (
        <div className="banner banner-error" role="alert">
          {props.error}
        </div>
      )}
      <form
        className="control-card session-form new-session-form"
        onSubmit={(event) => {
          event.preventDefault();
          props.onSubmit();
        }}
      >
        <label htmlFor="session-name">
          <span>Session name</span>
          <input
            id="session-name"
            type="text"
            required={true}
            maxLength={MAX_SESSION_NAME_CHARS}
            value={props.name}
            onChange={(event) => props.onNameChange(event.target.value)}
          />
        </label>
        <div className="range-inputs">
          <label htmlFor="new-start">
            <span>Start table</span>
            <input
              id="new-start"
              type="number"
              min={MIN_TABLE_ID}
              max={MAX_TABLE_ID}
              value={props.startValue}
              onChange={(event) => props.onStartChange(event.target.value)}
            />
          </label>
          <label htmlFor="new-end">
            <span>End table</span>
            <input
              id="new-end"
              type="number"
              min={MIN_TABLE_ID}
              max={MAX_TABLE_ID}
              value={props.endValue}
              onChange={(event) => props.onEndChange(event.target.value)}
            />
          </label>
        </div>
        {props.serverInfo?.pin_required === true ? (
          <label htmlFor="session-pin">
            <span>Session PIN</span>
            <input
              id="session-pin"
              type="password"
              value={props.pin}
              onChange={(event) => props.onPinChange(event.target.value)}
            />
          </label>
        ) : null}
        <ToggleSwitch
          id="collect-names"
          label="Collect viewer names"
          hint={collectNamesHint(props.collectNames)}
          checked={props.collectNames}
          onToggle={() => props.onCollectNamesChange(!props.collectNames)}
        />
        <SerialPicker {...props} />
        <div className="form-actions">
          <button type="submit" disabled={props.submitting}>
            Create session
          </button>
        </div>
      </form>
      <Footer serverInfo={props.serverInfo} />
    </main>
  );
}

function useLobbyCreateSocket() {
  const [state, dispatch] = useReducer(lobbyReducer, initialLobbyState);
  const socketRef = useRef<ReconnectingSocket | null>(null);
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
      },
      { pingMessage: () => JSON.stringify({ type: "ping" }) },
    );
    socketRef.current = socket;
    socket.connect();
    return () => socket.close();
  }, []);
  return { state, socketRef };
}

interface SubmitArgs {
  name: string;
  startValue: string;
  endValue: string;
  pin: string;
  collectNames: boolean;
  send: (msg: LobbyClientMsg) => boolean;
  setLocalError: (value: string | null) => void;
  setSubmitting: (value: boolean) => void;
}

function submitCreateSession(args: SubmitArgs): void {
  const normalized = normalizeSessionName(args.name);
  const start = Number.parseInt(args.startValue, PARSE_RADIX);
  const end = Number.parseInt(args.endValue, PARSE_RADIX);
  if (normalized === null) {
    args.setLocalError("Enter a session name of at most 80 characters.");
    return;
  }
  if (!isValidTableRange(start, end)) {
    args.setLocalError("Enter a valid table range (start <= end, 1-9999).");
    return;
  }
  const msg: LobbyClientMsg = {
    type: "create_session",
    name: normalized,
    start,
    end,
    pin: args.pin.length > 0 ? args.pin : null,
    collect_names: args.collectNames,
  };
  args.setLocalError(null);
  args.setSubmitting(true);
  if (!args.send(msg)) {
    args.setSubmitting(false);
    args.setLocalError("Not connected to the server yet. Try again in a moment.");
  }
}

function usePortPicker(setLocalError: (value: string | null) => void) {
  const [showAllPorts, setShowAllPorts] = useState(false);
  const [hasPendingPort, setHasPendingPort] = useState(false);
  const handleSelectPort = useCallback(async () => {
    try {
      const port = await requestMicrobitPort({ showAll: showAllPorts });
      setPendingSerialPort(port);
      setHasPendingPort(true);
      setLocalError(null);
    } catch {
      setLocalError("No receiver was selected.");
    }
  }, [showAllPorts, setLocalError]);
  return { showAllPorts, setShowAllPorts, hasPendingPort, handleSelectPort };
}

function useCreatedNavigation(created: { sessionId: string; creatorToken: string } | null): void {
  const navigate = useNavigate();
  useEffect(() => {
    if (created === null) {
      return;
    }
    saveCreatorToken(created.sessionId, created.creatorToken);
    bindPendingSerialPort(created.sessionId);
    Promise.resolve(navigate(`/s/${created.sessionId}`)).catch(() => undefined);
  }, [created, navigate]);
}

function useCreateSessionForm(): NewSessionViewProps {
  const { state, socketRef } = useLobbyCreateSocket();
  const [name, setName] = useState("");
  const [startValue, setStartValue] = useState(String(DEFAULT_RANGE_START));
  const [endValue, setEndValue] = useState(String(DEFAULT_RANGE_END));
  const [pin, setPin] = useState("");
  const [collectNames, setCollectNames] = useState(false);
  const [localError, setLocalError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const picker = usePortPicker(setLocalError);
  useCreatedNavigation(state.created);
  useEffect(() => {
    if (state.error !== null) {
      setSubmitting(false);
    }
  }, [state.error]);
  const handleSubmit = useCallback(() => {
    submitCreateSession({
      name,
      startValue,
      endValue,
      pin,
      collectNames,
      send: (msg) => socketRef.current?.send(msg) ?? false,
      setLocalError,
      setSubmitting,
    });
  }, [name, startValue, endValue, pin, collectNames, socketRef]);
  return {
    serverInfo: state.serverInfo,
    name,
    startValue,
    endValue,
    pin,
    collectNames,
    showAllPorts: picker.showAllPorts,
    serialSupported: isWebSerialSupported(),
    hasPendingPort: picker.hasPendingPort,
    error: localError ?? state.error?.message ?? null,
    submitting,
    onNameChange: setName,
    onStartChange: setStartValue,
    onEndChange: setEndValue,
    onPinChange: setPin,
    onCollectNamesChange: setCollectNames,
    onShowAllChange: picker.setShowAllPorts,
    onSelectPort: picker.handleSelectPort,
    onSubmit: handleSubmit,
  };
}

export function NewSessionPage() {
  const props = useCreateSessionForm();
  return <NewSessionView {...props} />;
}
