# 🐱 Charmaine Cat Studio

Designs, merchandise, digital collectibles, and AI-powered creations by Charmaine Cat.

Charmaine Cat Studio is a full-stack e-commerce and digital collectibles platform built to showcase modern software engineering, product design, AI integration, and payment processing.

The platform allows users to browse and purchase physical merchandise, digital collectibles, and AI-generated personalized artwork while interacting with the Charmaine Cat AI Assistant.

---

## ✨ Goals

This project is designed to demonstrate:

- Rust backend development
- Modern React frontend architecture
- Payment processing integrations
- AI-powered user experiences
- PostgreSQL database design
- Docker-based deployment
- Full-stack system design
- Product and UI/UX design skills

---

## 🚀 Features

### Storefront

- Product catalog
- Product detail pages
- Shopping cart
- Checkout workflow
- Order history
- User accounts

### Merchandise

- T-Shirts
- Stickers
- Posters
- Supporter Packs

### Digital Collectibles

Users can purchase digital collectibles created by Charmaine Cat Studio.

Each collectible includes:

- Unique collectible ID
- Digital ownership certificate
- High-resolution artwork
- Download access
- Purchase history

Example:

```text
Collectible #42

Title: Mango Mission
Edition: 42 / 100
Owner: CharmaineCatFan
Created: 2026-06-11
```

### AI Personalized Designs

Users can generate personalized versions of Charmaine Cat artwork.

Examples:

- Charmaine Cat Programmer Edition
- Mango Mission Edition
- Toronto Skyline Edition
- Bryan Wolf Adventure Edition
- Custom User Prompt Edition

### Charmaine Cat AI Assistant

Integrated AI assistant capable of:

- Product recommendations
- Design suggestions
- Order assistance
- FAQ support
- Personalized collectible generation

Example:

```text
User:
I like coding and cats.

Assistant:
You may enjoy the "Charmaine Cat Programmer Edition"
collectible and the Rust Developer T-Shirt.
```

---

## 💳 Payments

### Phase 1

- PayPal Checkout

### Phase 2

- Stripe Credit Card Payments
- Apple Pay
- Google Pay

### Future

- Subscription Supporter Membership
- Gift Cards
- Discount Codes

---

## 🏗 Architecture

```text
┌─────────────────────────────┐
│ React + TypeScript Frontend │
└──────────────┬──────────────┘
               │
               ▼
┌─────────────────────────────┐
│      Rust Axum Backend      │
└──────────────┬──────────────┘
               │
      ┌────────┴────────┐
      ▼                 ▼
┌─────────────┐   ┌─────────────┐
│ PostgreSQL  │   │ AI Services │
└─────────────┘   └─────────────┘
      │
      ▼
┌─────────────┐
│  Payments   │
│ PayPal      │
│ Stripe      │
└─────────────┘
```

---

## 🛠 Technology Stack

### Frontend

- React
- TypeScript
- Vite
- Tailwind CSS
- React Router
- TanStack Query

### Backend

- Rust
- Axum
- Tokio
- SQLx

### Database

- PostgreSQL

### AI

- OpenAI
- Ollama
- LangChain (optional)
- LangGraph (future)

### Payments

- PayPal
- Stripe

### DevOps

- Docker
- Docker Compose
- GitHub Actions

---

## 📦 Project Structure

```text
charmaine-cat-studio/
│
├── apps/
│   ├── storefront/
│   ├── api/
│   └── admin/
│
├── docs/
│
├── docker/
│
├── scripts/
│
├── assets/
│
└── README.md
```

---

## 🎨 Example Product Categories

### Merchandise

- Coding Cat T-Shirt
- Rust Developer Cat T-Shirt
- Mango Mission T-Shirt
- Bryan Wolf Collection

### Digital Collectibles

- Charmaine Cat #001
- Mango Mission Series
- Toronto Adventures Series
- Coding Journey Series

### Support Packages

- Buy Charmaine Cat a Mango
- Support Development
- Studio Supporter Pack

---

## 🤖 Future Roadmap

### Version 1

- Product catalog
- Shopping cart
- Checkout
- PayPal integration
- Admin dashboard

### Version 2

- AI-generated collectibles
- Stripe integration
- Personalized artwork generation

### Version 3

- AI shopping assistant
- Recommendation engine
- User collectible gallery

### Version 4

- Optional blockchain minting
- Marketplace trading
- Community features

---

## Performance Testing

Use Catbench to benchmark the public product list endpoint before and after the
in-memory product cache.

Before cache:

```bash
./catbench run \
  --url http://localhost:8080/products \
  --duration 30s \
  --concurrency 50 \
  --save results/products-before-cache.json
```

After cache:

```bash
./catbench run \
  --url http://localhost:8080/products \
  --duration 30s \
  --concurrency 50 \
  --save results/products-after-cache.json
```

Compare:

```bash
./catbench compare \
  results/products-before-cache.json \
  results/products-after-cache.json
```

---

## 🎯 Portfolio Highlights

This project demonstrates:

- Full-stack development
- Rust backend engineering
- Payment processing integration
- AI product integration
- Database design
- Authentication and authorization
- Cloud deployment
- Product design and branding

---

## 🐱 About Charmaine Cat

Charmaine Cat is a curious coding cat on a mission to build useful software, create fun designs, and collect as many mangos as possible before the fruit flies arrive.

```
Mission Status:
🍋 Mangoes Collected: Increasing
💻 Code Written: Always
🐱 Cuteness Level: Maximum
```
