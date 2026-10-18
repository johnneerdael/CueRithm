import React from "react";
import {createRoot} from "react-dom/client";
import App from "./App";
import "./brand/fonts.css";
import "./brand/tokens.css";
import "./styles.css";
createRoot(document.getElementById("root")!).render(<React.StrictMode><App/></React.StrictMode>);
