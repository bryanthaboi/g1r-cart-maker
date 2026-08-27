// Without this a render throw leaves an empty window, which reads as a crash.

import { Component, type ErrorInfo, type ReactNode } from "react";

interface Props {
  children: ReactNode;
}

interface State {
  error: Error | null;
  stack: string | null;
}

export class ErrorBoundary extends Component<Props, State> {
  override state: State = { error: null, stack: null };

  static getDerivedStateFromError(error: Error): Partial<State> {
    return { error };
  }

  override componentDidCatch(error: Error, info: ErrorInfo): void {
    console.error("render failed", error, info.componentStack);
    this.setState({ stack: info.componentStack ?? null });
  }

  override render(): ReactNode {
    const { error, stack } = this.state;
    if (!error) return this.props.children;
    return (
      <div className="crash">
        <h1>Something in this screen failed to draw</h1>
        <p>
          The rest of the app is still running. Go back to another tab, or reload the window. Your cart on disk is
          untouched.
        </p>
        <pre className="crash-message">{error.message}</pre>
        {stack ? <pre className="crash-stack">{stack.trim()}</pre> : null}
        <div className="crash-actions">
          <button type="button" onClick={() => this.setState({ error: null, stack: null })}>
            Try again
          </button>
          <button type="button" onClick={() => window.location.reload()}>
            Reload the window
          </button>
        </div>
      </div>
    );
  }
}
