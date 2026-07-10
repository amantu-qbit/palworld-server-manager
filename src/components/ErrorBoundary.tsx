import { Component } from "react";
import type { ErrorInfo, ReactNode } from "react";
import { TriangleAlert } from "lucide-react";

interface Props {
  children: ReactNode;
}
interface State {
  error: Error | null;
}

/** Catches render-time errors so an unexpected failure shows a message, not a blank window. */
export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    // Surface for diagnostics; kept minimal for a desktop app.
    console.error("Unhandled UI error:", error, info.componentStack);
  }

  render() {
    if (this.state.error) {
      return (
        <div className="errboundary">
          <div className="card card--pad errboundary__card">
            <div className="empty__icon empty__icon--error">
              <TriangleAlert size={24} />
            </div>
            <h3>Something went wrong</h3>
            <p>{this.state.error.message || "An unexpected error occurred."}</p>
            <button className="btn btn--primary" onClick={() => window.location.reload()}>
              Reload
            </button>
          </div>
        </div>
      );
    }
    return this.props.children;
  }
}
