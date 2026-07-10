import React from "react";
import ReactDOM from "react-dom/client";
import { QueryClientProvider } from "@tanstack/react-query";

import "@fontsource/geist-sans/400.css";
import "@fontsource/geist-sans/500.css";
import "@fontsource/geist-sans/600.css";
import "@fontsource/geist-mono/400.css";
import "@fontsource/geist-mono/500.css";
import "./styles/tokens.css";
import "./styles/global.css";
import "./styles/primitives.css";
import "./styles/layout.css";
import "./styles/screens.css";

import { queryClient } from "./hooks/queries";
import { PrefsProvider } from "./store/prefs";
import { ToastProvider } from "./hooks/useToast";
import { ConnectionProvider } from "./store/connection";
import { App } from "./App";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <PrefsProvider>
        <ToastProvider>
          <ConnectionProvider>
            <App />
          </ConnectionProvider>
        </ToastProvider>
      </PrefsProvider>
    </QueryClientProvider>
  </React.StrictMode>,
);
