import { useEffect, useRef } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import "@xterm/xterm/css/xterm.css";

type Props = { id: string; cwd: string; kind: "chat" | "shell"; visible: boolean; args?: string; onIdle?: () => void };

export default function Pane({ id, cwd, kind, visible, args, onIdle }: Props) {
  const host = useRef<HTMLDivElement>(null);
  const idleRef = useRef(onIdle);
  idleRef.current = onIdle;
  const fitRef = useRef<FitAddon | null>(null);
  const termRef = useRef<Terminal | null>(null);

  useEffect(() => {
    const term = new Terminal({
      fontFamily: "Cascadia Mono, Consolas, monospace",
      fontSize: 13,
      cursorBlink: true,
      theme: { background: "#0f0f10" },
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.loadAddon(new WebLinksAddon((_, uri) => openUrl(uri)));
    term.open(host.current!);
    fitRef.current = fit;
    termRef.current = term;

    let unlisten: UnlistenFn | undefined;
    let disposed = false;
    let unsignal: UnlistenFn | undefined;

    (async () => {
      unlisten = await listen<string>(`pty-data-${id}`, (e) => term.write(e.payload));
      // Hook Stop/Notification cua claude (xem pty.rs) -> claude xong viec hoac can user.
      unsignal = await listen(`claude-signal-${id}`, () => idleRef.current?.());
      if (disposed) {
        unlisten();
        unsignal();
        return;
      }
      fit.fit();
      await invoke("pty_spawn", { id, cwd, kind, cols: term.cols, rows: term.rows, args });
    })();

    // Paste anh: luu ra file tam roi dan duong dan vao terminal, claude tu nhan anh.
    const onPaste = async (e: ClipboardEvent) => {
      const img = [...(e.clipboardData?.items ?? [])].find((i) => i.type.startsWith("image/"));
      const file = img?.getAsFile();
      if (!file) return;
      e.preventDefault();
      e.stopImmediatePropagation();
      const path = await invoke<string>("save_image", new Uint8Array(await file.arrayBuffer()));
      term.paste(`"${path}" `);
    };
    host.current!.addEventListener("paste", onPaste, true);

    const d1 = term.onData((data) => invoke("pty_write", { id, data }));
    const d2 = term.onResize(({ cols, rows }) => invoke("pty_resize", { id, cols, rows }));
    const ro = new ResizeObserver(() => fit.fit());
    ro.observe(host.current!);
    const el = host.current!;

    return () => {
      disposed = true;
      ro.disconnect();
      el.removeEventListener("paste", onPaste, true);
      d1.dispose();
      d2.dispose();
      unlisten?.();
      unsignal?.();
      invoke("pty_kill", { id });
      term.dispose();
    };
  }, [id]);

  useEffect(() => {
    if (visible) {
      requestAnimationFrame(() => {
        fitRef.current?.fit();
        if (kind === "chat") termRef.current?.focus();
      });
    }
  }, [visible]);

  return <div ref={host} className="pane" />;
}
