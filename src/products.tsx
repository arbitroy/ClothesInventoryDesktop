import { useEffect, useState } from "react";
import { call, notify, type Product } from "./api";

export default function Products() {
  const [list, setList] = useState<Product[]>([]);
  const [name, setName] = useState("");
  const [size, setSize] = useState("");
  const [edit, setEdit] = useState<Product | null>(null);

  const load = async () =>
    setList((await call<Product[]>("list_products_with_stock")) ?? []);
  useEffect(() => {
    load();
  }, []);

  async function add() {
    if (!name.trim()) return;
    const id = await call<string>("create_product", {
      name: name.trim(),
      size: size.trim() || null,
      notes: null,
    });
    if (id === undefined) return;
    setName("");
    setSize("");
    load();
  }

  async function save() {
    if (!edit || !edit.name.trim()) return;
    const changed = await call<number>("update_product", { product: edit });
    if (changed === undefined) return;
    if (changed === 0)
      notify(
        "Someone else changed this product first, so your edit was not saved.",
        "warn",
      );
    setEdit(null);
    load();
  }

  async function remove(p: Product) {
    if (!confirm(`Delete ${p.name}?`)) return;
    if ((await call<number>("delete_product", { id: p.id })) !== undefined)
      load();
  }

  return (
    <section>
      <h2>Products</h2>
      <div className="row">
        <input
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="Product name"
        />
        <input
          value={size}
          onChange={(e) => setSize(e.target.value)}
          placeholder="Size (optional)"
        />
        <button onClick={add}>Add product</button>
      </div>
      <table>
        <thead>
          <tr>
            <th>Product</th>
            <th>Size</th>
            <th className="num">In stock</th>
            <th />
          </tr>
        </thead>
        <tbody>
          {list.map((p) =>
            edit?.id === p.id ? (
              <tr key={p.id}>
                <td>
                  <input
                    value={edit.name}
                    onChange={(e) => setEdit({ ...edit, name: e.target.value })}
                  />
                </td>
                <td>
                  <input
                    value={edit.size ?? ""}
                    onChange={(e) =>
                      setEdit({ ...edit, size: e.target.value || null })
                    }
                  />
                </td>
                <td className="num">{p.stock}</td>
                <td className="actions">
                  <button onClick={save}>Save</button>
                  <button className="ghost" onClick={() => setEdit(null)}>
                    Cancel
                  </button>
                </td>
              </tr>
            ) : (
              <tr key={p.id}>
                <td>{p.name}</td>
                <td>{p.size ?? "-"}</td>
                <td className={"num" + (p.stock < 0 ? " bad" : "")}>
                  {p.stock}
                </td>
                <td className="actions">
                  <button className="ghost" onClick={() => setEdit(p)}>
                    Edit
                  </button>
                  <button className="ghost" onClick={() => remove(p)}>
                    Delete
                  </button>
                </td>
              </tr>
            ),
          )}
          {!list.length && (
            <tr>
              <td colSpan={4} className="empty">
                No products yet. Add your first one above.
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </section>
  );
}
