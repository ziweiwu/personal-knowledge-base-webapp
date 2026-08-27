import { useEffect, useId, useRef, useState } from 'react';
import { Modal } from './Modal';

interface PromptDialogProps {
  title: string;
  label: string;
  initialValue?: string;
  hint?: string;
  submitLabel: string;
  busy?: boolean;
  error?: string | null;
  /**
   * Preselect the initial value on open, so typing replaces it. `'stem'` leaves
   * the extension out of the selection and therefore intact.
   */
  preselect?: 'stem' | 'all';
  onSubmit: (value: string) => void;
  onCancel: () => void;
}

/** Where the extension starts. A leading dot is part of the name, not an extension. */
function stemEnd(name: string): number {
  const dot = name.lastIndexOf('.');
  return dot > 0 ? dot : name.length;
}

export function PromptDialog({
  title,
  label,
  initialValue = '',
  hint,
  submitLabel,
  busy,
  error,
  preselect,
  onSubmit,
  onCancel,
}: PromptDialogProps) {
  const [value, setValue] = useState(initialValue);
  const inputId = useId();
  const hintId = useId();
  const inputRef = useRef<HTMLInputElement>(null);
  const trimmed = value.trim();

  // Runs after <Modal>'s focus trap has focused the field — child effects settle
  // before the parent's — so the selection is not undone by the focus that follows.
  useEffect(() => {
    const input = inputRef.current;
    if (!preselect || !input) return;
    input.setSelectionRange(0, preselect === 'stem' ? stemEnd(input.value) : input.value.length);
  }, [preselect]);

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
          ref={inputRef}
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
