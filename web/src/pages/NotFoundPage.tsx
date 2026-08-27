import { Link } from 'react-router-dom';

export function NotFoundPage() {
  return (
    <div className="state">
      <p className="state__title">Page not found</p>
      <p className="state__detail">That address does not match any screen in this app.</p>
      <Link className="btn" to="/">
        Go to my documents
      </Link>
    </div>
  );
}
