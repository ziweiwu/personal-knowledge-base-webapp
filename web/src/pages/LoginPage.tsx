import { useEffect, useId, useState, type FormEvent, type InputHTMLAttributes } from 'react';
import { Navigate, useLocation, useNavigate } from 'react-router-dom';
import { ApiRequestError } from '../api/client';
import { ThemeToggle } from '../components/layout/ThemeToggle';
import { Spinner } from '../components/ui/States';
import { useAuth } from '../state/auth-context';

/**
 * Never distinguishes "no such account" from "wrong password": that difference
 * is exactly what an attacker enumerating addresses is looking for.
 */
const GENERIC_FAILURE = 'Incorrect email or password.';
const LOCKED_OUT =
  'Too many sign-in attempts. This account is locked for a short while — wait a few minutes and try again.';
const UNREACHABLE = 'Could not reach the server. Check your connection and try again.';

function signInFailure(cause: unknown): string {
  if (!(cause instanceof ApiRequestError)) return UNREACHABLE;
  if (cause.isRateLimited) return LOCKED_OUT;
  if (cause.isUnauthorized) return GENERIC_FAILURE;
  return cause.message || GENERIC_FAILURE;
}

/** Owns the request and everything the form shows about it, not the field values. */
function useSignIn(from: string) {
  const { signIn } = useAuth();
  const navigate = useNavigate();
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const submit = async (email: string, password: string): Promise<boolean> => {
    setSubmitting(true);
    setError(null);
    try {
      await signIn(email, password);
      void navigate(from, { replace: true });
      return true;
    } catch (cause) {
      setError(signInFailure(cause));
      return false;
    } finally {
      setSubmitting(false);
    }
  };

  return { error, submitting, submit };
}

type CredentialFieldProps = {
  id: string;
  label: string;
  value: string;
  onValueChange: (next: string) => void;
} & InputHTMLAttributes<HTMLInputElement>;

function CredentialField({ id, label, value, onValueChange, ...inputProps }: CredentialFieldProps) {
  return (
    <div className="field">
      <label className="field__label" htmlFor={id}>
        {label}
      </label>
      <input
        id={id}
        className="input"
        value={value}
        onChange={(event) => onValueChange(event.target.value)}
        {...inputProps}
      />
    </div>
  );
}

function LoginHeading() {
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
      <h1 className="login__brand" style={{ flex: 1 }}>
        kbviewer
      </h1>
      <ThemeToggle />
    </div>
  );
}

function FormError({ id, message }: { id: string; message: string | null }) {
  if (!message) return null;
  return (
    <p className="form-error" id={id} role="alert">
      {message}
    </p>
  );
}

function LoginForm({ from }: { from: string }) {
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const { error, submitting, submit } = useSignIn(from);

  const emailId = useId();
  const passwordId = useId();
  const errorId = useId();
  const describedBy = error ? errorId : undefined;

  const onSubmit = async (event: FormEvent) => {
    event.preventDefault();
    const signedIn = await submit(email, password);
    if (!signedIn) setPassword('');
  };

  return (
    <form className="login__card" onSubmit={(event) => void onSubmit(event)} noValidate>
      <LoginHeading />
      <p className="login__sub">Sign in to browse your documents.</p>
      <FormError id={errorId} message={error} />

      <CredentialField
        id={emailId}
        label="Email"
        value={email}
        onValueChange={setEmail}
        type="email"
        name="email"
        autoComplete="username"
        autoCapitalize="none"
        autoCorrect="off"
        spellCheck={false}
        required
        disabled={submitting}
        aria-describedby={describedBy}
        autoFocus
      />

      <CredentialField
        id={passwordId}
        label="Password"
        value={password}
        onValueChange={setPassword}
        type="password"
        name="password"
        autoComplete="current-password"
        required
        disabled={submitting}
        aria-describedby={describedBy}
      />

      <button type="submit" className="btn btn--primary btn--block" disabled={submitting}>
        {submitting ? <Spinner label="Signing in" /> : 'Sign in'}
      </button>
    </form>
  );
}

export function LoginPage() {
  const { status } = useAuth();
  const location = useLocation();
  const from = (location.state as { from?: string } | null)?.from ?? '/';

  useEffect(() => {
    document.title = 'Sign in · kbviewer';
  }, []);

  if (status === 'authenticated') return <Navigate to={from} replace />;

  return (
    <div className="login">
      <LoginForm from={from} />
    </div>
  );
}
