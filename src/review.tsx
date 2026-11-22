import { useEffect, useState } from "react";
import { call, fmtDate, type Flag } from "./api";

export default function Review() {
    const [flags, setFlags] = useState<Flag[]>([]);
    const load = async () =>
        setFlags((await call<Flag[]>("list_pending_flags")) ?? []);
    useEffect(() => {
        load();
    }, []);

    async function resolve(f: Flag, approve: boolean) {
        if ((await call("resolve_flag", { flagId: f.id, approve })) !== undefined)
            load();
    }

    return (
        <section>
            <h2>Review flags</h2>
            <p className="hint">
                Approve means the sale is fine as recorded. Reject means it was wrong,
                so record the correction as an adjustment.
            </p>
            {flags.map((f) => (
                <div className="card" key={f.id}>
                    <div>
                        <strong>{f.productName}</strong>
                        <div className="hint">
                            Sold {f.transactionQuantity} on {fmtDate(f.transactionTimestamp)}.
                            Stock is now <span className="bad">{f.stockNow}</span>.
                        </div>
                    </div>
                    <div className="actions">
                        <button onClick={() => resolve(f, true)}>Approve</button>
                        <button className="ghost" onClick={() => resolve(f, false)}>
                            Reject
                        </button>
                    </div>
                </div>
            ))}
            {!flags.length && <p className="empty">Nothing to review.</p>}
        </section>
    );
}
