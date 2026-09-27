import { useEffect, useId, useRef, useState } from 'react';
import { Modal } from './Modal';
import { Button } from './Button';
import { FormError } from './States';

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
  /**
   * What is wrong with the typed name, or `null`. Reported as the user types and blocks
   * submission, so a name the server would refuse never costs a round trip.
   */
  validate?: (value: string) => string | null;
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
  validate,
  onSubmit,
  onCancel,
}: PromptDialogProps) {
  const [value, setValue] = useState(initialValue);
  const inputId = useId();
  const hintId = useId();
  const inputRef = useRef<HTMLInputElement>(null);
  const trimmed = value.trim();
  // The typed name outranks whatever the last attempt failed on: the user has moved on.
  const problem = trimmed.length > 0 && validate ? validate(trimmed) : null;

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
          <Button onClick={onCancel} disabled={busy}>
            Cancel
          </Button>
          <Button
            type="submit"
            form={`${inputId}-form`}
            variant="primary"
            disabled={busy || trimmed.length === 0 || problem !== null}
          >
            {busy ? 'Working…' : submitLabel}
          </Button>
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
          aria-invalid={problem !== null || undefined}
          data-autofocus
        />
        {hint ? (
          <p className="state__detail state__detail--start" id={hintId}>
            {hint}
          </p>
        ) : null}
        <FormError message={problem ?? error ?? null} />
      </form>
    </Modal>
  );
}
