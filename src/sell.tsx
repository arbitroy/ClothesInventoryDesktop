import { useEffect, useState } from "react";
import { call, notify, wholeNumber, type Product } from "./api";

export default function Sell() {
    const [products, setProducts] = useState<Product[]>([]);
    const [productId, setProductId] = useState("");
    const [qty, setQty] = useState("1");
    const [reason, setReason] = useState("sales");
    const [price, setPrice] = useState("");

    const load = async () =>
        setProducts((await call<Product[]>("list_products_with_stock")) ?? []);
    useEffect(() => {
        load();
    }, []);
    const chosen = products.find((p) => p.id === productId);

    async function submit() {
        if (!chosen || !wholeNumber(qty) || Number(qty) < 1) return;
        const cents = price.trim() ? Math.round(Number(price) * 100) : null;
        if (cents !== null && !(cents >= 0)) return;
        const q = Number(qty);
        const id = await call<string>("record_out", {
            productId: chosen.id,
            quantity: q,
            reason,
            pricePerUnit: cents,
        });
        if (id === undefined) return;
        if (chosen.stock - q < 0)
            notify(
                `Recorded, but ${chosen.name} is now below zero. It was flagged for review.`,
                "warn",
            );
        else notify("Recorded.");
        setQty("1");
        setPrice("");
        load();
    }

    const total =
        price.trim() && Number(price) >= 0
            ? Number(price) * Number(qty || 0)
            : null;

    return (
        <section>
            <h2>Stock out</h2>
            <div className="form">
                <label>
                    Product
                    <select
                        value={productId}
                        onChange={(e) => setProductId(e.target.value)}
                    >
                        <option value="">Choose product</option>
                        {products.map((p) => (
                            <option key={p.id} value={p.id}>
                                {p.name}
                                {p.size ? ` (${p.size})` : ""}
                            </option>
                        ))}
                    </select>
                </label>
                {chosen && (
                    <p className={"hint" + (chosen.stock <= 0 ? " bad" : "")}>
                        In stock: {chosen.stock}
                    </p>
                )}
                <label>
                    Quantity
                    <input
                        type="number"
                        min={1}
                        value={qty}
                        onChange={(e) => setQty(e.target.value)}
                    />
                </label>
                <label>
                    Reason
                    <select value={reason} onChange={(e) => setReason(e.target.value)}>
                        <option value="sales">Sale</option>
                        <option value="damage">Damaged</option>
                        <option value="adjustment">Adjustment</option>
                    </select>
                </label>
                <label>
                    Price per unit in KSh (optional)
                    <input
                        type="number"
                        min={0}
                        step="any"
                        value={price}
                        onChange={(e) => setPrice(e.target.value)}
                    />
                </label>
                {total !== null && (
                    <p className="hint">Total: KSh {total.toLocaleString()}</p>
                )}
                <button onClick={submit} disabled={!chosen}>
                    Record stock out
                </button>
            </div>
        </section>
    );
}
