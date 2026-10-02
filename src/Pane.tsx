import { useEffect, useRef } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import "@xterm/xterm/css/xterm.css";

// Duong dan tuyet doi (H:\x), tuong doi (./x, a/b). ponytail: khong ho tro path co dau cach hay bi wrap sang dong sau.
const PATH_RE = /(?:(?<!\w)[A-Za-z]:[\\/]|\.{1,2}[\\/]|(?<![\w:\\/.-])[\w.-]+[\\/])[^\s"'`<>|*?]*/g;

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
      theme: { background: kind === "chat" ? "#222222" : "#2a2a2a", foreground: "#c8c8c8", cursor: "#c8c8c8", selectionBackground: "#4a4a4a" },
      // ConPTY tu reflow khi resize; de xterm reflow nua thi banner/khung claude bi in lap.
      windowsPty: { backend: "conpty", buildNumber: 22631 },
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.loadAddon(new WebLinksAddon((_, uri) => openUrl(uri)));
    // Ctrl+click vao path -> file mo trong Notepad++ (nhay toi dong neu co :line), thu muc mo File Explorer.
    term.registerLinkProvider({
      provideLinks(y, cb) {
        const text = term.buffer.active.getLine(y - 1)?.translateToString(true) ?? "";
        const links = [...text.matchAll(PATH_RE)].map((m) => {
          const raw = m[0].replace(/[.,:;)\]]+$/, "");
          return {
            text: raw,
            range: { start: { x: m.index! + 1, y }, end: { x: m.index! + raw.length, y } },
            activate(e: MouseEvent) {
              if (!e.ctrlKey) return;
              // Tach hau to :line / :line-line / :line:col (vd constants.ts:179-185).
              const [, file, line] = raw.match(/^(.*?)(?::(\d+)(?:[-:]\d+)*)?$/)!;
              let p = file.replace(/\//g, "\\").replace(/(?<!:)\\+$/, "");
              if (!/^[A-Za-z]:/.test(p)) p = `${cwd}\\${p}`;
              invoke("open_in_editor", { path: p, line: line ? +line : null })
                .catch(() => revealItemInDir(p))
                .catch(() => {});
            },
          };
        });
        cb(links.length ? links : undefined);
      },
    });
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
    // Debounce: keo splitter / bat split ban ra hang chuc resize, moi lan claude ve lai ca man hinh.
    let t: number | undefined;
    const ro = new ResizeObserver(() => {
      clearTimeout(t);
      t = window.setTimeout(() => el.offsetWidth && el.offsetHeight && fit.fit(), 80);
    });
    const el = host.current!;
    ro.observe(el);

    return () => {
      disposed = true;
      clearTimeout(t);
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
