import { useState } from "react";
import { call, notify, today, wholeNumber, type PreviewRow } from "./api";

const valid = (v?: string) => wholeNumber(v) && Number(v) >= 0;

export default function DailyClose() {
    const [date, setDate] = useState(today());
    const [rows, setRows] = useState<PreviewRow[]>([]);
    const [counts, setCounts] = useState<Record<string, string>>({});
    const [notes, setNotes] = useState<Record<string, string>>({});
    const [by, setBy] = useState(localStorage.getItem("closedBy") ?? "");
    const [loaded, setLoaded] = useState(false);

    async function load() {
        const r = await call<PreviewRow[]>("day_preview", { date });
        setRows(r ?? []);
        setCounts({});
        setNotes({});
        setLoaded(r !== undefined);
    }

    const ready =
        loaded &&
        rows.length > 0 &&
        by.trim() !== "" &&
        rows.every((r) => valid(counts[r.productId]));

    async function close() {
        if (
            !confirm(
                `Close ${date}? Stock will be set to your counts and this can't be undone.`,
            )
        )
            return;
        localStorage.setItem("closedBy", by.trim());
        const payload = rows.map((r) => ({
            productId: r.productId,
            counted: Number(counts[r.productId]),
            notes: notes[r.productId]?.trim() || null,
        }));
        if (
            (await call<number>("close_day", {
                date,
                counts: payload,
                closedBy: by.trim(),
            })) === undefined
        )
            return;
        notify(`${date} closed. Stock now matches your count.`);
        setRows([]);
        setLoaded(false);
    }

    return (
        <section>
            <h2>Daily close</h2>
            <div className="row">
                <input
                    type="date"
                    value={date}
                    onChange={(e) => {
                        setDate(e.target.value);
                        setLoaded(false);
                        setRows([]);
                    }}
                />
                <button onClick={load}>Load day</button>
            </div>
            {loaded && (
                <>
                    <table>
                        <thead>
                            <tr>
                                <th>Product</th>
                                <th className="num">Opening</th>
                                <th className="num">In</th>
                                <th className="num">Out</th>
                                <th className="num">Expected</th>
                                <th>Counted</th>
                                <th className="num">Difference</th>
                                <th>Note</th>
                            </tr>
                        </thead>
                        <tbody>
                            {rows.map((r) => {
                                const c = counts[r.productId];
                                const diff = valid(c) ? Number(c) - r.expected : null;
                                return (
                                    <tr key={r.productId}>
                                        <td>{r.productName}</td>
                                        <td className="num">{r.opening}</td>
                                        <td className="num">{r.totalIn}</td>
                                        <td className="num">{r.totalOut}</td>
                                        <td className="num">{r.expected}</td>
                                        <td>
                                            <input
                                                className="short"
                                                type="number"
                                                min={0}
                                                value={c ?? ""}
                                                onChange={(e) =>
                                                    setCounts({
                                                        ...counts,
                                                        [r.productId]: e.target.value,
                                                    })
                                                }
                                            />
                                        </td>
                                        <td className={"num" + (diff ? " bad" : "")}>
                                            {diff === null ? "-" : diff > 0 ? `+${diff}` : diff}
                                        </td>
                                        <td>
                                            <input
                                                value={notes[r.productId] ?? ""}
                                                onChange={(e) =>
                                                    setNotes({ ...notes, [r.productId]: e.target.value })
                                                }
                                                placeholder="Optional"
                                            />
                                        </td>
                                    </tr>
                                );
                            })}
                            {!rows.length && (
                                <tr>
                                    <td colSpan={8} className="empty">
                                        No products to count.
                                    </td>
                                </tr>
                            )}
                        </tbody>
                    </table>
                    <div className="row">
                        <input
                            value={by}
                            onChange={(e) => setBy(e.target.value)}
                            placeholder="Closed by (your name)"
                        />
                        <button onClick={close} disabled={!ready}>
                            Close day
                        </button>
                    </div>
                    {!ready && (
                        <p className="hint">
                            Enter a count for every product and your name to close the day.
                        </p>
                    )}
                </>
            )}
        </section>
    );
}
