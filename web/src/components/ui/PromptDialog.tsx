import { useId, useState } from 'react';
import { Modal } from './Modal';

interface PromptDialogProps {
  title: string;
  label: string;
  initialValue?: string;
  hint?: string;
  submitLabel: string;
  busy?: boolean;
  error?: string | null;
  onSubmit: (value: string) => void;
  onCancel: () => void;
}

export function PromptDialog({
  title,
  label,
  initialValue = '',
  hint,
  submitLabel,
  busy,
  error,
  onSubmit,
  onCancel,
}: PromptDialogProps) {
  const [value, setValue] = useState(initialValue);
  const inputId = useId();
  const hintId = useId();
  const trimmed = value.trim();

  return (
    <Modal
      title={title}
      onClose={onCancel}
      dismissOnBackdrop={false}
      footer={
        <>
          <button type="button" className="btn" onClick={onCancel} disabled={busy}>
            Cancel
          </button>
          <button
            type="submit"
            form={`${inputId}-form`}
            className="btn btn--primary"
            disabled={busy || trimmed.length === 0}
          >
            {busy ? 'Working…' : submitLabel}
          </button>
        </>
      }
    >
      <form
        id={`${inputId}-form`}
        className="field"
        onSubmit={(event) => {
          event.preventDefault();
          if (trimmed) onSubmit(trimmed);
        }}
      >
        <label className="field__label" htmlFor={inputId}>
          {label}
        </label>
        <input
          id={inputId}
          className="input"
          value={value}
          onChange={(event) => setValue(event.target.value)}
          disabled={busy}
          autoComplete="off"
          spellCheck={false}
          aria-describedby={hint ? hintId : undefined}
          data-autofocus
        />
        {hint ? (
          <p className="state__detail" id={hintId} style={{ textAlign: 'left' }}>
            {hint}
          </p>
        ) : null}
        {error ? <p className="form-error">{error}</p> : null}
      </form>
    </Modal>
  );
}
