import { Link, Route, Routes, useParams } from "react-router-dom";
import { useEffect, useState } from "react";
import { useCart } from "./context/CartContext";
import "./App.css";

type Product = {
  id: string;
  slug: string;
  name: string;
  description: string;
  price_cents: number;
  image_url: string | null;
  category: string;
};

function formatPrice(cents: number) {
  return `$${(cents / 100).toFixed(2)} CAD`;
}

function Layout() {
  const { totalItems } = useCart();

  return (
    <>
      <header className="topbar">
        <Link to="/" className="brand">
          🐱 Charmaine Cat Studio
        </Link>

        <Link to="/checkout" className="cartLink">
          Cart ({totalItems})
        </Link>
      </header>

      <Routes>
        <Route path="/" element={<ProductListPage />} />
        <Route path="/products/:slug" element={<ProductDetailPage />} />
        <Route path="/checkout" element={<CheckoutPage />} />
      </Routes>
    </>
  );
}

function ProductListPage() {
  const [products, setProducts] = useState<Product[]>([]);

  useEffect(() => {
    fetch("http://localhost:8080/products")
      .then((res) => res.json())
      .then(setProducts)
      .catch(console.error);
  }, []);

  return (
    <>
      <section className="hero">
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
              <Link className="buttonLink" to={`/products/${product.slug}`}>
                View
              </Link>
            </div>
          </article>
        ))}
      </section>
    </>
  );
}

function ProductDetailPage() {
  const { slug } = useParams();
  const { addToCart } = useCart();
  const [product, setProduct] = useState<Product | null>(null);

  useEffect(() => {
    fetch(`http://localhost:8080/products/${slug}`)
      .then((res) => {
        if (!res.ok) {
          throw new Error("Product not found");
        }
        return res.json();
      })
      .then(setProduct)
      .catch(console.error);
  }, [slug]);

  if (!product) {
    return <p>Loading product...</p>;
  }

  return (
    <section className="detail">
      <Link to="/" className="backLink">
        ← Back to studio
      </Link>

      <div className="detailImage">🐱</div>

      <div>
        <p className="category">{product.category}</p>
        <h1>{product.name}</h1>
        <p>{product.description}</p>
        <strong className="price">{formatPrice(product.price_cents)}</strong>

        <button onClick={() => addToCart(product)}>Add to cart</button>
      </div>
    </section>
  );
}

function CheckoutPage() {
  const { items, subtotalCents, removeFromCart, clearCart } = useCart();
  const [isCreatingOrder, setIsCreatingOrder] = useState(false);
  const [orderId, setOrderId] = useState<string | null>(null);
  const [checkoutError, setCheckoutError] = useState<string | null>(null);

  async function handleCheckout() {
    setIsCreatingOrder(true);
    setCheckoutError(null);

    try {
      const response = await fetch("http://localhost:8080/orders", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          items: items.map((item) => ({
            product_id: item.id,
            quantity: item.quantity,
          })),
        }),
      });

      if (!response.ok) {
        throw new Error("Failed to create order");
      }

      const data = await response.json();

      setOrderId(data.order_id);
      clearCart();
    } catch (error) {
      console.error(error);
      setCheckoutError("Could not create order. Please try again.");
    } finally {
      setIsCreatingOrder(false);
    }
  }

  return (
    <section className="checkout">
      <h1>Your Cart</h1>

      {orderId && (
        <div className="emptyCart">
          <p className="successText">Order created successfully.</p>
          <p>{orderId}</p>
          <Link to="/" className="buttonLink">
            Continue shopping
          </Link>
        </div>
      )}

      {!orderId && items.length === 0 && (
        <div className="emptyCart">
          <p>Your cart is empty.</p>
          <Link to="/" className="buttonLink">
            Browse products
          </Link>
        </div>
      )}

      {!orderId && items.length > 0 && (
        <>
          <div className="cartItems">
            {items.map((item) => (
              <article className="cartItem" key={item.id}>
                <div>
                  <h2>{item.name}</h2>
                  <p>Quantity: {item.quantity}</p>
                  <p>{formatPrice(item.price_cents)} each</p>
                </div>

                <button onClick={() => removeFromCart(item.id)}>Remove</button>
              </article>
            ))}
          </div>

          <div className="checkoutSummary">
            <strong>Subtotal: {formatPrice(subtotalCents)}</strong>

            <button onClick={handleCheckout} disabled={isCreatingOrder}>
              {isCreatingOrder ? "Creating order..." : "Checkout"}
            </button>

            <button className="secondaryButton" onClick={clearCart}>
              Clear cart
            </button>

            {checkoutError && <p className="errorText">{checkoutError}</p>}
          </div>
        </>
      )}
    </section>
  );
}

function App() {
  return (
    <main className="page">
      <Layout />
    </main>
  );
}

export default App;