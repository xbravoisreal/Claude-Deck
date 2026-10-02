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
    <div className="meter" title={`${fmtTokens(tokens)} tokens (local logs)`} style={{ borderColor: color, background: `${color}22` }}>
      <div className="fill" style={{ width: `${pct ?? 0}%`, background: `${color}66` }} />
      <span className="meter-label">{label}</span>
      <span className="meter-pct">{pct != null ? `${pct.toFixed(0)}%` : "–"}</span>
      <span className="meter-reset">{fmtReset(reset)}</span>
    </div>
  );
}

type Accounts = { current: string | null; saved: { email: string; plan: string }[] };

// Doi account claude: chi chat mo SAU khi doi moi dung account moi.
function AccountMenu({ onSwitch }: { onSwitch: () => void }) {
  const [a, setA] = useState<Accounts | null>(null);
  const [menu, setMenu] = useState(false);
  const refresh = () => invoke<Accounts>("list_accounts").then(setA).catch(() => setA(null));
  useEffect(() => void refresh(), []);

  const toggle = () => (menu ? setMenu(false) : (refresh(), setMenu(true)));
  const pick = async (email: string) => {
    await invoke("switch_account", { email });
    setMenu(false);
    refresh();
    onSwitch();
  };
  const remove = async (email: string) => (await invoke("remove_account", { email }), refresh());

  return (
    <div className="acct">
      <button onClick={toggle} title="Quan ly account">👤 {a?.current ?? "chua dang nhap"} ▴</button>
      {menu && a && (
        <div className="acct-menu">
          {a.saved.map((s) => (
            <div key={s.email} className={`acct-item${s.email === a.current ? " on" : ""}`}>
              <button onClick={() => s.email !== a.current && pick(s.email)}>
                {s.email} <span className="plan">{s.plan}</span>
              </button>
              {s.email !== a.current && <button className="x" title="Xoa" onClick={() => remove(s.email)}>✕</button>}
            </div>
          ))}
          <div className="hint">Them account: go /login trong chat, mo lai menu nay. Doi account roi bam ＋ (chat moi) de dung.</div>
        </div>
      )}
    </div>
  );
}

export default function UsageFooter() {
  const [u, setU] = useState<Usage | null>(null);

  // usage API loi tam thoi (429, token het han) -> giu % cu thay vi rot ve "–"
  const tick = () =>
    invoke<Usage>("get_usage")
      .then((n) => setU((p) => (n.source === "logs" && p?.source === "oauth" ? { ...p, five_hour_tokens: n.five_hour_tokens, seven_day_tokens: n.seven_day_tokens } : n)))
      .catch(() => {});
  const onSwitch = () => (setU(null), tick());
  useEffect(() => {
    tick();
    const t = setInterval(tick, 60_000);
    return () => clearInterval(t);
  }, []);

  return (
    <footer className="footer">
      {u && (
        <>
          <Meter label="5h" pct={u.five_hour_pct} reset={u.five_hour_reset} tokens={u.five_hour_tokens} />
          <Meter label="1w" pct={u.seven_day_pct} reset={u.seven_day_reset} tokens={u.seven_day_tokens} okColor="#388bfd" />
        </>
      )}
      <AccountMenu onSwitch={onSwitch} />
    </footer>
  );
}
