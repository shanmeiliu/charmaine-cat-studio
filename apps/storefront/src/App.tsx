import { Link, Route, Routes, useNavigate, useParams } from "react-router-dom";
import { useCallback, useEffect, useRef, useState } from "react";
import type { FormEvent, ReactNode } from "react";
import { useCart } from "./context/CartContext";
import "./App.css";

const API_BASE_URL = "http://localhost:8080";
const PRODUCT_CATEGORY_OPTIONS = [
  "Merchandise",
  "Digital Collectible",
  "Support",
  "Sticker",
  "Poster",
  "Apparel",
  "Accessory",
  "Other",
];
const CUSTOM_CATEGORY_OPTION = "Other";

type Product = {
  id: string;
  slug: string;
  name: string;
  description: string;
  price_cents: number;
  image_url: string | null;
  category: string;
};

type AdminProduct = Product & {
  active: boolean;
};

type ProductFormValues = {
  slug: string;
  name: string;
  description: string;
  price_cents: string;
  categoryOption: string;
  customCategory: string;
  image_url: string;
  active: boolean;
};

type OrderDetail = {
  id: string;
  status: string;
  total_cents: number;
  items: {
    id: string;
    product_id: string;
    product_name: string;
    unit_price_cents: number;
    quantity: number;
    line_total_cents: number;
  }[];
};

type AdminOrder = {
  id: string;
  status: string;
  total_cents: number;
  created_at: string;
  updated_at: string;
};

type PayPalButtonsOptions = {
  createOrder: () => Promise<string>;
  onApprove: (data: { orderID: string }) => Promise<void>;
  onCancel: () => void;
  onError: (error: unknown) => void;
  style: {
    color: string;
    label: string;
    layout: string;
    shape: string;
  };
};

type PayPalButtons = {
  render: (container: HTMLElement) => Promise<void>;
  close: () => Promise<void>;
};

declare global {
  interface Window {
    paypal?: {
      Buttons: (options: PayPalButtonsOptions) => PayPalButtons;
    };
  }
}

function formatPrice(cents: number) {
  return `$${(cents / 100).toFixed(2)} CAD`;
}

function formatDate(value: string) {
  return new Intl.DateTimeFormat("en-CA", {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(value));
}

function shortOrderId(id: string) {
  return id.slice(0, 8);
}

function productToFormValues(product: AdminProduct): ProductFormValues {
  const isDefaultCategory =
    product.category !== CUSTOM_CATEGORY_OPTION &&
    PRODUCT_CATEGORY_OPTIONS.includes(product.category);

  return {
    slug: product.slug,
    name: product.name,
    description: product.description,
    price_cents: String(product.price_cents),
    categoryOption: isDefaultCategory
      ? product.category
      : CUSTOM_CATEGORY_OPTION,
    customCategory: isDefaultCategory ? "" : product.category,
    image_url: product.image_url ?? "",
    active: product.active,
  };
}

function emptyProductFormValues(): ProductFormValues {
  return {
    slug: "",
    name: "",
    description: "",
    price_cents: "",
    categoryOption: "Merchandise",
    customCategory: "",
    image_url: "",
    active: true,
  };
}

function categoryFromForm(values: ProductFormValues) {
  return values.categoryOption === CUSTOM_CATEGORY_OPTION
    ? values.customCategory.trim()
    : values.categoryOption.trim();
}

function isProductFormValid(values: ProductFormValues) {
  return (
    values.slug.trim() !== "" &&
    values.name.trim() !== "" &&
    Number(values.price_cents) > 0 &&
    categoryFromForm(values) !== ""
  );
}

function productPayloadFromForm(values: ProductFormValues) {
  return {
    slug: values.slug.trim(),
    name: values.name.trim(),
    description: values.description,
    price_cents: Number(values.price_cents),
    category: categoryFromForm(values),
    image_url: values.image_url.trim() || null,
    active: values.active,
  };
}

function loadPayPalScript(clientId: string) {
  if (window.paypal) {
    return Promise.resolve();
  }

  const existingScript = document.querySelector<HTMLScriptElement>(
    'script[data-paypal-sdk="true"]',
  );

  if (existingScript) {
    return new Promise<void>((resolve, reject) => {
      existingScript.addEventListener("load", () => resolve(), { once: true });
      existingScript.addEventListener(
        "error",
        () => reject(new Error("PayPal SDK failed to load")),
        { once: true },
      );
    });
  }

  return new Promise<void>((resolve, reject) => {
    const script = document.createElement("script");
    script.src = `https://www.paypal.com/sdk/js?client-id=${encodeURIComponent(clientId)}&currency=CAD&components=buttons`;
    script.async = true;
    script.dataset.paypalSdk = "true";
    script.addEventListener("load", () => resolve(), { once: true });
    script.addEventListener(
      "error",
      () => reject(new Error("PayPal SDK failed to load")),
      { once: true },
    );
    document.head.appendChild(script);
  });
}

function Layout() {
  const { totalItems } = useCart();

  return (
    <>
      <header className="topbar">
        <Link to="/" className="brand">
          🐱 Charmaine Cat Studio
        </Link>

        <nav className="topNav" aria-label="Primary navigation">
          <Link to="/" className="navLink">
            Shop
          </Link>
          <Link to="/admin/orders" className="navLink">
            Admin Orders
          </Link>
          <Link to="/admin/products" className="navLink">
            Admin Products
          </Link>
          <Link to="/checkout" className="cartLink">
            Cart ({totalItems})
          </Link>
        </nav>
      </header>

      <Routes>
        <Route path="/" element={<ProductListPage />} />
        <Route path="/products/:slug" element={<ProductDetailPage />} />
        <Route path="/checkout" element={<CheckoutPage />} />
        <Route path="/orders/:id" element={<OrderConfirmationPage />} />
        <Route path="/admin/orders" element={<AdminOrdersPage />} />
        <Route path="/admin/products" element={<AdminProductsPage />} />
      </Routes>
    </>
  );
}

function ProductListPage() {
  const [products, setProducts] = useState<Product[]>([]);

  useEffect(() => {
    fetch(`${API_BASE_URL}/products`)
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
    fetch(`${API_BASE_URL}/products/${slug}`)
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
  const [checkoutError, setCheckoutError] = useState<string | null>(null);
  const navigate = useNavigate();

  async function handleCheckout() {
    setIsCreatingOrder(true);
    setCheckoutError(null);

    try {
      const response = await fetch(`${API_BASE_URL}/orders`, {
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

      clearCart();
      navigate(`/orders/${data.order_id}`);
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

      {items.length === 0 && (
        <div className="emptyCart">
          <p>Your cart is empty.</p>
          <Link to="/" className="buttonLink">
            Browse products
          </Link>
        </div>
      )}

      {items.length > 0 && (
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

function OrderConfirmationPage() {
  const { id } = useParams();
  const [order, setOrder] = useState<OrderDetail | null>(null);
  const [orderError, setOrderError] = useState<string | null>(null);

  const loadOrder = useCallback(async () => {
    setOrderError(null);

    try {
      const response = await fetch(`${API_BASE_URL}/orders/${id}`);

      if (!response.ok) {
        throw new Error("Order not found");
      }

      setOrder(await response.json());
    } catch (error) {
      console.error(error);
      setOrderError("Could not load this order.");
    }
  }, [id]);

  useEffect(() => {
    void loadOrder();
  }, [loadOrder]);

  if (orderError) {
    return <p className="errorText">{orderError}</p>;
  }

  if (!order) {
    return <p>Loading order...</p>;
  }

  return (
    <section className="checkout">
      <h1>Order Created</h1>

      <div className="emptyCart">
        <p className="successText">Thank you for supporting Charmaine Cat.</p>
        <p>
          Order ID: <strong>{order.id}</strong>
        </p>
        <p>
          Status: <strong>{order.status}</strong>
        </p>
        <p>
          Total: <strong>{formatPrice(order.total_cents)}</strong>
        </p>

        {order.status === "pending" && (
          <PayPalPayment orderId={order.id} onPaid={loadOrder} />
        )}

        <Link to="/" className="buttonLink">
          Continue shopping
        </Link>
      </div>

      <div className="cartItems" style={{ marginTop: 24 }}>
        {order.items.map((item) => (
          <article className="cartItem" key={item.id}>
            <div>
              <h2>{item.product_name}</h2>
              <p>Quantity: {item.quantity}</p>
              <p>{formatPrice(item.unit_price_cents)} each</p>
            </div>

            <strong>{formatPrice(item.line_total_cents)}</strong>
          </article>
        ))}
      </div>
    </section>
  );
}

function PayPalPayment({
  orderId,
  onPaid,
}: {
  orderId: string;
  onPaid: () => Promise<void>;
}) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [paymentError, setPaymentError] = useState<string | null>(null);

  useEffect(() => {
    const clientId = import.meta.env.VITE_PAYPAL_CLIENT_ID;
    let buttons: PayPalButtons | undefined;
    let cancelled = false;

    if (!clientId) {
      setPaymentError("PayPal is not configured.");
      return;
    }

    async function renderButtons() {
      try {
        await loadPayPalScript(clientId);

        if (cancelled || !window.paypal || !containerRef.current) {
          return;
        }

        buttons = window.paypal.Buttons({
          style: {
            color: "gold",
            label: "paypal",
            layout: "vertical",
            shape: "pill",
          },
          async createOrder() {
            setPaymentError(null);

            const response = await fetch(
              `${API_BASE_URL}/payments/paypal/create-order`,
              {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({ order_id: orderId }),
              },
            );

            if (!response.ok) {
              throw new Error("Could not start PayPal checkout");
            }

            const result = await response.json();
            return result.paypal_order_id;
          },
          async onApprove(data) {
            const response = await fetch(
              `${API_BASE_URL}/payments/paypal/capture-order`,
              {
                method: "POST",
                headers: { "Content-Type": "application/json" },
                body: JSON.stringify({
                  order_id: orderId,
                  paypal_order_id: data.orderID,
                }),
              },
            );

            if (!response.ok) {
              throw new Error("PayPal payment could not be captured");
            }

            await onPaid();
          },
          onCancel() {
            setPaymentError("PayPal checkout was cancelled.");
          },
          onError(error) {
            console.error(error);
            setPaymentError("PayPal payment failed. Please try again.");
          },
        });

        await buttons.render(containerRef.current);
      } catch (error) {
        console.error(error);
        setPaymentError("PayPal payment failed. Please try again.");
      }
    }

    void renderButtons();

    return () => {
      cancelled = true;
      if (buttons) {
        void buttons.close();
      }
    };
  }, [onPaid, orderId]);

  return (
    <div className="paypalPayment">
      <p>Complete your payment securely with PayPal Sandbox.</p>
      <div ref={containerRef} className="paypalButtons" />
      {paymentError && <p className="errorText">{paymentError}</p>}
    </div>
  );
}

function AdminOrdersPage() {
  const [orders, setOrders] = useState<AdminOrder[]>([]);
  const [ordersError, setOrdersError] = useState<string | null>(null);
  const [updatingOrderId, setUpdatingOrderId] = useState<string | null>(null);

  useEffect(() => {
    async function loadOrders() {
      setOrdersError(null);

      try {
        const response = await fetch(`${API_BASE_URL}/admin/orders`);

        if (!response.ok) {
          throw new Error("Could not load admin orders");
        }

        const data: { orders: AdminOrder[] } = await response.json();
        setOrders(data.orders);
      } catch (error) {
        console.error(error);
        setOrdersError("Could not load orders.");
      }
    }

    void loadOrders();
  }, []);

  async function updateOrderStatus(orderId: string, status: AdminOrder["status"]) {
    setUpdatingOrderId(orderId);
    setOrdersError(null);

    try {
      const response = await fetch(`${API_BASE_URL}/admin/orders/${orderId}/status`, {
        method: "PATCH",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({ status }),
      });

      if (!response.ok) {
        throw new Error("Could not update order status");
      }

      const updatedOrder: AdminOrder = await response.json();
      setOrders((currentOrders) =>
        currentOrders.map((order) =>
          order.id === updatedOrder.id ? updatedOrder : order,
        ),
      );
    } catch (error) {
      console.error(error);
      setOrdersError("Could not update this order.");
    } finally {
      setUpdatingOrderId(null);
    }
  }

  return (
    <section className="adminOrders">
      <div className="adminHeader">
        <div>
          <p className="category">Merchant dashboard</p>
          <h1>Orders</h1>
        </div>
      </div>

      {ordersError && <p className="errorText">{ordersError}</p>}

      <div className="adminOrderList">
        <div className="adminOrderRow adminOrderHead">
          <span>Order</span>
          <span>Status</span>
          <span>Total</span>
          <span>Created</span>
          <span>Actions</span>
        </div>

        {orders.map((order) => {
          const isUpdating = updatingOrderId === order.id;
          const canShip = order.status === "paid";
          const canComplete = order.status === "paid" || order.status === "shipped";
          const canCancel = order.status !== "completed" && order.status !== "cancelled";

          return (
            <article className="adminOrderRow" key={order.id}>
              <strong>#{shortOrderId(order.id)}</strong>
              <span className={`statusBadge status-${order.status}`}>
                {order.status}
              </span>
              <span>{formatPrice(order.total_cents)}</span>
              <span>{formatDate(order.created_at)}</span>
              <div className="adminActions">
                {canShip && (
                  <button
                    disabled={isUpdating}
                    onClick={() => void updateOrderStatus(order.id, "shipped")}
                  >
                    Mark Shipped
                  </button>
                )}
                {canComplete && (
                  <button
                    disabled={isUpdating}
                    onClick={() => void updateOrderStatus(order.id, "completed")}
                  >
                    Mark Completed
                  </button>
                )}
                {canCancel && (
                  <button
                    className="secondaryButton"
                    disabled={isUpdating}
                    onClick={() => void updateOrderStatus(order.id, "cancelled")}
                  >
                    Cancel
                  </button>
                )}
              </div>
            </article>
          );
        })}
      </div>

      {orders.length === 0 && !ordersError && (
        <div className="emptyCart">
          <p>No orders yet.</p>
        </div>
      )}
    </section>
  );
}

function AdminProductsPage() {
  const [products, setProducts] = useState<AdminProduct[]>([]);
  const [productError, setProductError] = useState<string | null>(null);
  const [createForm, setCreateForm] = useState<ProductFormValues>(
    emptyProductFormValues,
  );
  const [editProductId, setEditProductId] = useState<string | null>(null);
  const [editForm, setEditForm] = useState<ProductFormValues>(
    emptyProductFormValues,
  );
  const [savingProductId, setSavingProductId] = useState<string | null>(null);
  const [isCreatingProduct, setIsCreatingProduct] = useState(false);

  useEffect(() => {
    async function loadProducts() {
      setProductError(null);

      try {
        const response = await fetch(`${API_BASE_URL}/admin/products`);

        if (!response.ok) {
          throw new Error("Could not load admin products");
        }

        setProducts(await response.json());
      } catch (error) {
        console.error(error);
        setProductError("Could not load products.");
      }
    }

    void loadProducts();
  }, []);

  async function createProduct(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setProductError(null);
    setIsCreatingProduct(true);

    if (!isProductFormValid(createForm)) {
      setProductError("Could not create product. Check the required fields.");
      setIsCreatingProduct(false);
      return;
    }

    try {
      const response = await fetch(`${API_BASE_URL}/admin/products`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify(productPayloadFromForm(createForm)),
      });

      if (!response.ok) {
        throw new Error("Could not create product");
      }

      const product: AdminProduct = await response.json();
      setProducts((currentProducts) => [product, ...currentProducts]);
      setCreateForm(emptyProductFormValues());
    } catch (error) {
      console.error(error);
      setProductError("Could not create product. Check the required fields.");
    } finally {
      setIsCreatingProduct(false);
    }
  }

  async function updateProduct(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    if (!editProductId) {
      return;
    }

    setProductError(null);
    setSavingProductId(editProductId);

    if (!isProductFormValid(editForm)) {
      setProductError("Could not update product. Check the required fields.");
      setSavingProductId(null);
      return;
    }

    try {
      const response = await fetch(
        `${API_BASE_URL}/admin/products/${editProductId}`,
        {
          method: "PATCH",
          headers: {
            "Content-Type": "application/json",
          },
          body: JSON.stringify(productPayloadFromForm(editForm)),
        },
      );

      if (!response.ok) {
        throw new Error("Could not update product");
      }

      const updatedProduct: AdminProduct = await response.json();
      setProducts((currentProducts) =>
        currentProducts.map((product) =>
          product.id === updatedProduct.id ? updatedProduct : product,
        ),
      );
      setEditProductId(null);
      setEditForm(emptyProductFormValues());
    } catch (error) {
      console.error(error);
      setProductError("Could not update product. Check the required fields.");
    } finally {
      setSavingProductId(null);
    }
  }

  async function toggleProductActive(product: AdminProduct) {
    setProductError(null);
    setSavingProductId(product.id);

    try {
      const response = await fetch(
        `${API_BASE_URL}/admin/products/${product.id}/active`,
        {
          method: "PATCH",
          headers: {
            "Content-Type": "application/json",
          },
          body: JSON.stringify({ active: !product.active }),
        },
      );

      if (!response.ok) {
        throw new Error("Could not update product active status");
      }

      const updatedProduct: AdminProduct = await response.json();
      setProducts((currentProducts) =>
        currentProducts.map((currentProduct) =>
          currentProduct.id === updatedProduct.id
            ? updatedProduct
            : currentProduct,
        ),
      );
    } catch (error) {
      console.error(error);
      setProductError("Could not update product active status.");
    } finally {
      setSavingProductId(null);
    }
  }

  function startEditing(product: AdminProduct) {
    setEditProductId(product.id);
    setEditForm(productToFormValues(product));
  }

  return (
    <section className="adminProducts">
      <div className="adminHeader">
        <div>
          <p className="category">Merchant dashboard</p>
          <h1>Products</h1>
        </div>
      </div>

      {productError && <p className="errorText">{productError}</p>}

      <ProductForm
        title="Create product"
        values={createForm}
        onChange={setCreateForm}
        onSubmit={createProduct}
        submitLabel={isCreatingProduct ? "Creating..." : "Create Product"}
        disabled={isCreatingProduct}
      />

      <div className="adminProductList">
        <div className="adminProductRow adminProductHead">
          <span>Product</span>
          <span>Slug</span>
          <span>Category</span>
          <span>Price</span>
          <span>Status</span>
          <span>Actions</span>
        </div>

        {products.map((product) => {
          const isSaving = savingProductId === product.id;
          const isEditing = editProductId === product.id;

          return (
            <article className="adminProductRow" key={product.id}>
              {isEditing ? (
                <div className="adminProductEdit">
                  <ProductForm
                    title={`Edit ${product.name}`}
                    values={editForm}
                    onChange={setEditForm}
                    onSubmit={updateProduct}
                    submitLabel={isSaving ? "Saving..." : "Save Product"}
                    disabled={isSaving}
                    secondaryAction={
                      <button
                        className="secondaryButton"
                        type="button"
                        disabled={isSaving}
                        onClick={() => setEditProductId(null)}
                      >
                        Cancel Edit
                      </button>
                    }
                  />
                </div>
              ) : (
                <>
                  <strong>{product.name}</strong>
                  <span>{product.slug}</span>
                  <span>{product.category}</span>
                  <span>{formatPrice(product.price_cents)}</span>
                  <span
                    className={`statusBadge ${
                      product.active ? "status-paid" : "status-cancelled"
                    }`}
                  >
                    {product.active ? "active" : "inactive"}
                  </span>
                  <div className="adminActions">
                    <button
                      disabled={isSaving}
                      onClick={() => startEditing(product)}
                    >
                      Edit
                    </button>
                    <button
                      className="secondaryButton"
                      disabled={isSaving}
                      onClick={() => void toggleProductActive(product)}
                    >
                      {product.active ? "Deactivate" : "Activate"}
                    </button>
                  </div>
                </>
              )}
            </article>
          );
        })}
      </div>

      {products.length === 0 && !productError && (
        <div className="emptyCart">
          <p>No products yet.</p>
        </div>
      )}
    </section>
  );
}

function ProductForm({
  title,
  values,
  onChange,
  onSubmit,
  submitLabel,
  disabled,
  secondaryAction,
}: {
  title: string;
  values: ProductFormValues;
  onChange: (values: ProductFormValues) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
  submitLabel: string;
  disabled: boolean;
  secondaryAction?: ReactNode;
}) {
  function updateField<Field extends keyof ProductFormValues>(
    field: Field,
    value: ProductFormValues[Field],
  ) {
    onChange({ ...values, [field]: value });
  }

  return (
    <form className="adminProductForm" onSubmit={onSubmit}>
      <h2>{title}</h2>

      <label>
        <span>Slug</span>
        <input
          required
          value={values.slug}
          onChange={(event) => updateField("slug", event.target.value)}
          placeholder="coding-cat-shirt"
        />
      </label>

      <label>
        <span>Name</span>
        <input
          required
          value={values.name}
          onChange={(event) => updateField("name", event.target.value)}
          placeholder="Coding Cat Shirt"
        />
      </label>

      <label>
        <span>Category</span>
        <select
          required
          value={values.categoryOption}
          onChange={(event) =>
            updateField("categoryOption", event.target.value)
          }
        >
          {PRODUCT_CATEGORY_OPTIONS.map((category) => (
            <option key={category} value={category}>
              {category}
            </option>
          ))}
        </select>
      </label>

      <label>
        <span>Price cents</span>
        <input
          required
          min="1"
          type="number"
          value={values.price_cents}
          onChange={(event) => updateField("price_cents", event.target.value)}
          placeholder="2500"
        />
      </label>

      {values.categoryOption === CUSTOM_CATEGORY_OPTION && (
        <label>
          <span>Custom category</span>
          <input
            required
            value={values.customCategory}
            onChange={(event) =>
              updateField("customCategory", event.target.value)
            }
            placeholder="Custom category"
          />
        </label>
      )}

      <label className="adminWideField">
        <span>Description</span>
        <textarea
          value={values.description}
          onChange={(event) => updateField("description", event.target.value)}
          rows={3}
          placeholder="Short product description"
        />
      </label>

      <label className="adminWideField">
        <span>Image URL</span>
        <input
          value={values.image_url}
          onChange={(event) => updateField("image_url", event.target.value)}
          placeholder="https://example.com/product.png"
        />
      </label>

      <label className="adminCheckbox">
        <input
          checked={values.active}
          type="checkbox"
          onChange={(event) => updateField("active", event.target.checked)}
        />
        <span>Active</span>
      </label>

      <div className="adminFormActions">
        <button disabled={disabled} type="submit">
          {submitLabel}
        </button>
        {secondaryAction}
      </div>
    </form>
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
