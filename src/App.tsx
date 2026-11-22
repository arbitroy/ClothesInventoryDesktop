import { useEffect, useState } from "react";
import "./App.css";
import { onMessage } from "./api";
import Products from "./products";
import Receipts from "./Receipts";
import Sell from "./sell";
import Review from "./review";
import DailyClose from "./dailyclose";

const screens = { Products, Receipts, "Stock out": Sell, "Review flags": Review, "Daily close": DailyClose };
type Screen = keyof typeof screens;

export default function App() {
  const [tab, setTab] = useState<Screen>("Products");
  const [msg, setMsg] = useState<{ text: string; kind: string } | null>(null);

  useEffect(() => {
    let timer: number | undefined;
    onMessage((text, kind) => {
      setMsg({ text, kind });
      clearTimeout(timer);
      timer = window.setTimeout(() => setMsg(null), 6000);
    });
  }, []);

  const Current = screens[tab];
  return (
    <div className="app">
      <nav>
        <h1>Clothes inventory</h1>
        {(Object.keys(screens) as Screen[]).map((k) => (
          <button key={k} className={k === tab ? "on" : ""} onClick={() => setTab(k)}>{k}</button>
        ))}
      </nav>
      <main>
        {msg && <div className={"banner " + msg.kind} onClick={() => setMsg(null)}>{msg.text}</div>}
        <Current products={[]} />
      </main>
    </div>
  );
}