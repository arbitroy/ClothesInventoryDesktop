import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

type Product = {
  id: string;
  name: string;
  size: string | null;
  notes: string | null;
  version: number;
  createdAt: number;
  updatedAt: number;
  deletedAt: number | null;
};

export default function App() {
  const [products, setProducts] = useState<Product[]>([]);
  const [name, setName] = useState("");

  async function load() {
    setProducts(await invoke<Product[]>("list_products"));
  }

  async function add() {
    if (!name.trim()) return;
    await invoke("create_product", { name, size: null, notes: null });
    setName("");
    load();
  }

async function deleteProduct(id: string) {
  try {
    await invoke("delete_product", { id });
    load();
  } catch (e) {
    alert(`Delete failed: ${e}`);
  }
}

  useEffect(() => {
    load();
  }, []);

  return (
    <main style={{ padding: 24 }}>
      <h1>Products</h1>
      <input
        value={name}
        onChange={(e) => setName(e.target.value)}
        placeholder="Product name"
      />
      <button onClick={add}>Add</button>
      <ul>
        {products.map((p) => (
          <li key={p.id} >
            {p.name} V({p.version})<button onClick={() => deleteProduct(p.id)}>Delete</button>
          </li>
        ))}
      </ul>
    </main>
  );
}
