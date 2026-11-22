import { useEffect, useState } from "react";
import { call, notify } from "./api";

type Pairing = { code: string; address: string | null };
type Device = {
    id: string;
    name: string;
    pairedAt: number;
    lastSeenAt: number | null;
    revoked: boolean;
};
type Fields = { name?: string; size?: string | null; notes?: string | null };
type Conflict = {
    id: string;
    deviceId: string;
    createdAt: number;
    desktop: Fields & { deleted?: boolean };
    incoming: Fields;
};

const when = (ms: number | null) =>
    ms ? new Date(ms).toLocaleString() : "never";

function Version({ title, f }: { title: string; f: Fields }) {
    return (
        <div>
            <div className="hint">{title}</div>
            <strong>{f.name}</strong>
            <div className="hint">
                Size: {f.size ?? "-"} · Notes: {f.notes ?? "-"}
            </div>
        </div>
    );
}

export default function Sync() {
    const [pairing, setPairing] = useState<Pairing | null>(null);
    const [devices, setDevices] = useState<Device[]>([]);
    const [conflicts, setConflicts] = useState<Conflict[]>([]);

    const load = async () => {
        setDevices((await call<Device[]>("list_devices")) ?? []);
        setConflicts((await call<Conflict[]>("list_pending_conflicts")) ?? []);
    };
    useEffect(() => {
        load();
    }, []);

    // while a code is on screen, keep checking so the new phone appears in the list
    useEffect(() => {
        if (!pairing) return;
        const t = setInterval(load, 3000);
        return () => clearInterval(t);
    }, [pairing]);

    async function startPairing() {
        const p = await call<Pairing>("start_pairing");
        if (p) setPairing(p);
    }

    async function revoke(d: Device) {
        if (!confirm(`Revoke ${d.name}? It stops syncing immediately.`)) return;
        if ((await call<number>("revoke_device", { id: d.id })) !== undefined) {
            notify(`${d.name} revoked.`);
            load();
        }
    }

    async function resolve(c: Conflict, usePhone: boolean) {
        if (
            (await call("resolve_conflict", { conflictId: c.id, usePhone })) !==
            undefined
        ) {
            notify("Resolved. The phone gets the result on its next sync.");
            load();
        }
    }

    return (
        <section>
            <h2>Sync</h2>

            <h3>Pair a phone</h3>
            {!pairing ? (
                <div className="row">
                    <button onClick={startPairing}>Pair a phone</button>
                </div>
            ) : (
                <div className="card stack">
                    <div>
                        <div className="hint">
                            Enter this code in the phone app. It works once and expires in 10
                            minutes.
                        </div>
                        <div className="code">{pairing.code}</div>
                        <div className="hint">
                            {pairing.address ? (
                                <>
                                    Computer address: <strong>{pairing.address}</strong>.{" "}
                                </>
                            ) : null}
                            The phone must be on the same Wi-Fi.
                        </div>
                    </div>
                    <div className="row">
                        <button className="ghost" onClick={() => setPairing(null)}>
                            Done
                        </button>
                    </div>
                </div>
            )}

            <h3>Paired phones</h3>
            <table>
                <thead>
                    <tr>
                        <th>Phone</th>
                        <th>Paired</th>
                        <th>Last sync</th>
                        <th>Status</th>
                        <th />
                    </tr>
                </thead>
                <tbody>
                    {devices.map((d) => (
                        <tr key={d.id}>
                            <td>{d.name}</td>
                            <td>{when(d.pairedAt)}</td>
                            <td>{when(d.lastSeenAt)}</td>
                            <td>
                                {d.revoked ? <span className="bad">Revoked</span> : "Active"}
                            </td>
                            <td className="actions">
                                {!d.revoked && (
                                    <button className="ghost" onClick={() => revoke(d)}>
                                        Revoke
                                    </button>
                                )}
                            </td>
                        </tr>
                    ))}
                    {!devices.length && (
                        <tr>
                            <td colSpan={5} className="empty">
                                No phones paired yet.
                            </td>
                        </tr>
                    )}
                </tbody>
            </table>

            <h3>Sync conflicts</h3>
            <p className="hint">
                The phone edited a product that was changed here first. Nothing was
                overwritten. Pick which version to keep.
            </p>
            {conflicts.map((c) => (
                <div className="card stack" key={c.id}>
                    <div className="two">
                        <Version
                            title={
                                c.desktop.deleted
                                    ? "On this computer (deleted)"
                                    : "On this computer"
                            }
                            f={c.desktop}
                        />
                        <Version title="From the phone" f={c.incoming} />
                    </div>
                    <div className="row">
                        <button onClick={() => resolve(c, false)}>
                            Keep this computer's version
                        </button>
                        <button
                            className="ghost"
                            disabled={c.desktop.deleted}
                            onClick={() => resolve(c, true)}
                        >
                            Use the phone's version
                        </button>
                    </div>
                </div>
            ))}
            {!conflicts.length && <p className="empty">No conflicts.</p>}
        </section>
    );
}
