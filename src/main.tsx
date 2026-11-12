import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import ChampionRankOverlay from "./features/ChampionRankOverlay";
import OcrOverlay from "./features/augment-picker/OcrOverlay";
import ShopOverlay from "./features/ShopOverlay";
import "./styles/tokens.css";
import "./features/champion-rank-overlay.css";
import "./features/augment-picker/ocr-overlay.css";

const isOcrOverlay = new URLSearchParams(window.location.search).get("window") === "ocr-overlay";
const isChampionRankOverlay = new URLSearchParams(window.location.search).get("window") === "champion-rank-overlay";
const isShopOverlay = new URLSearchParams(window.location.search).get("window") === "shop-overlay";
if (isOcrOverlay || isShopOverlay) {
  document.documentElement.classList.add("ocr-overlay-html");
  document.body.classList.add("ocr-overlay-body");
}
if (isChampionRankOverlay) {
  document.documentElement.classList.add("champion-rank-overlay-html");
  document.body.classList.add("champion-rank-overlay-body");
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    {isShopOverlay ? <ShopOverlay /> : isOcrOverlay ? <OcrOverlay /> : isChampionRankOverlay ? <ChampionRankOverlay /> : <App />}
  </React.StrictMode>,
);
