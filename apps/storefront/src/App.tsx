import { useEffect, useState } from "react";
import "./App.css";

type Product = {
  id: string;
  name: string;
  description: string;
  price_cents: number;
  image_url: string;
  category: string;
};

function formatPrice(cents: number) {
  return `$${(cents / 100).toFixed(2)} CAD`;
}

function App() {
  const [products, setProducts] = useState<Product[]>([]);

  useEffect(() => {
    fetch("http://localhost:8080/products")
      .then((res) => res.json())
      .then(setProducts)
      .catch(console.error);
  }, []);

  return (
    <main className="page">
      <section className="hero">
        <p className="eyebrow">🐱 Charmaine Cat Studio</p>
        <h1>Merch, digital collectibles, and cute coding cat designs.</h1>
        <p>
          Support Charmaine Cat by buying a virtual mango, a T-shirt, or a
          collectible design.
        </p>
      </section>

      <section className="grid">
        {products.map((product) => (
          <article className="card" key={product.id}>
            <div className="imagePlaceholder">🐱</div>
            <p className="category">{product.category}</p>
            <h2>{product.name}</h2>
            <p>{product.description}</p>
            <div className="cardFooter">
              <strong>{formatPrice(product.price_cents)}</strong>
              <button>Add to cart</button>
            </div>
          </article>
        ))}
      </section>
    </main>
  );
}

export default App;