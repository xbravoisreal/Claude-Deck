import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

type Usage = {
  source: "oauth" | "logs";
  five_hour_pct: number | null;
  five_hour_reset: string | null;
  seven_day_pct: number | null;
  seven_day_reset: string | null;
  five_hour_tokens: number;
  seven_day_tokens: number;
};

function fmtReset(iso?: string | null) {
  if (!iso) return "";
  const m = Math.round((new Date(iso).getTime() - Date.now()) / 60000);
  if (m <= 0) return "vua reset";
  const d = Math.floor(m / 1440);
  const h = Math.floor((m % 1440) / 60);
  return d > 0 ? `${d}d ${h}h` : `${h}h ${m % 60}m`;
}

function fmtTokens(n: number) {
  return n >= 1e6 ? `${(n / 1e6).toFixed(1)}M` : n >= 1e3 ? `${Math.round(n / 1e3)}k` : `${n}`;
}

function Meter(props: { label: string; pct: number | null; reset: string | null; tokens: number; okColor?: string }) {
  const { label, pct, reset, tokens, okColor = "#3fb950" } = props;
  const color = pct == null ? "#666666" : pct >= 90 ? "#e5484d" : pct >= 70 ? "#f5a524" : okColor;
  return (
    <div className="meter" style={{ borderColor: color, background: `${color}22` }}>
      <div className="fill" style={{ width: `${pct ?? 0}%`, background: `${color}66` }} />
      <span className="meter-label">{label}</span>
      <span className="meter-pct">{pct != null ? `${pct.toFixed(0)}%` : `${fmtTokens(tokens)} tok`}</span>
      <span className="meter-reset">{fmtReset(reset)}</span>
    </div>
  );
}

export default function UsageFooter() {
  const [u, setU] = useState<Usage | null>(null);

  useEffect(() => {
    const tick = () => invoke<Usage>("get_usage").then(setU).catch(() => setU(null));
    tick();
    const t = setInterval(tick, 60_000);
    return () => clearInterval(t);
  }, []);

  return (
    <footer className="footer">
      {u ? (
        <>
          <Meter label="5h" pct={u.five_hour_pct} reset={u.five_hour_reset} tokens={u.five_hour_tokens} />
          <Meter label="1w" pct={u.seven_day_pct} reset={u.seven_day_reset} tokens={u.seven_day_tokens} okColor="#388bfd" />
          <span className="src">{u.source === "oauth" ? "usage API" : "local logs (uoc tinh)"}</span>
        </>
      ) : (
        <span className="src">usage: chua co du lieu</span>
      )}
    </footer>
  );
}
