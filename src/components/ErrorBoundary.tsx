import { Component, type ErrorInfo, type ReactNode } from "react";

interface ErrorBoundaryProps {
  children: ReactNode;
  context: string;
}

interface ErrorBoundaryState {
  failed: boolean;
}

/** Prevents a non-critical UI subtree from blanking the entire app. */
export class ErrorBoundary extends Component<
  ErrorBoundaryProps,
  ErrorBoundaryState
> {
  state: ErrorBoundaryState = { failed: false };

  static getDerivedStateFromError(): ErrorBoundaryState {
    return { failed: true };
  }

  componentDidCatch(error: Error, info: ErrorInfo): void {
    console.error(
      `Error rendering ${this.props.context}:`,
      error,
      info.componentStack,
    );
  }

  componentDidUpdate(prevProps: ErrorBoundaryProps): void {
    if (prevProps.context !== this.props.context && this.state.failed) {
      this.setState({ failed: false });
    }
  }

  render(): ReactNode {
    if (this.state.failed) {
      return (
        <div className="p-4 text-sm text-stone-500">
          Unable to load {this.props.context}. Check logs or retry.
        </div>
      );
    }
    return this.props.children;
  }
}
