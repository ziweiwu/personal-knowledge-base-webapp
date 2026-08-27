import { Component, type ErrorInfo, type ReactNode } from 'react';

interface Props {
  children: ReactNode;
}

interface State {
  error: Error | null;
}

/**
 * Last line of defence around the app.
 *
 * Rendering runs over document HTML produced from arbitrary files, and a single unexpected
 * throw in a layout effect otherwise unmounts the entire React root and leaves a blank
 * page with no way back. A visible error with a reload is always better than a white
 * screen that looks like the server died.
 */
export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error('Unhandled error in the UI', error, info);
  }

  render() {
    if (!this.state.error) return this.props.children;
    return (
      <div className="fatal" role="alert">
        <h1>Something went wrong displaying this page</h1>
        <p>The rest of your documents are unaffected.</p>
        <pre>{this.state.error.message}</pre>
        <button type="button" className="btn btn--primary" onClick={() => window.location.reload()}>
          Reload
        </button>
      </div>
    );
  }
}
