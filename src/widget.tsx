import React from "react";
import ReactDOM from "react-dom/client";
import "./index.css";

function Widget() {
  return <div className="p-2 text-xs text-slate-200">widget</div>;
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <Widget />
  </React.StrictMode>,
);
