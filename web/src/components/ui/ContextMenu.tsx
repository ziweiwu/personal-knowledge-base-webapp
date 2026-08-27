import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { useEscapeKey } from '../../hooks/useEscapeKey';

export interface MenuItem {
  id: string;
  label: string;
  icon?: string;
  danger?: boolean;
  onSelect: () => void;
}

interface ContextMenuProps {
  items: MenuItem[];
  /** Screen coordinates of the control that opened the menu. */
  anchor: { x: number; y: number };
  label: string;
  onClose: () => void;
}

const MENU_MARGIN = 8;

export function ContextMenu({ items, anchor, label, onClose }: ContextMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState(anchor);
  const [activeIndex, setActiveIndex] = useState(0);

  useEscapeKey(onClose);

  // Flip the menu back inside the viewport once its real size is known.
  useLayoutEffect(() => {
    const menu = menuRef.current;
    if (!menu) return;
    const { width, height } = menu.getBoundingClientRect();
    const x = Math.min(anchor.x, window.innerWidth - width - MENU_MARGIN);
    const y = Math.min(anchor.y, window.innerHeight - height - MENU_MARGIN);
    setPosition({ x: Math.max(MENU_MARGIN, x), y: Math.max(MENU_MARGIN, y) });
  }, [anchor]);

  useEffect(() => {
    const buttons = menuRef.current?.querySelectorAll<HTMLButtonElement>('.menu__item');
    buttons?.[activeIndex]?.focus();
  }, [activeIndex]);

  useEffect(() => {
    const dismiss = (event: MouseEvent) => {
      if (!menuRef.current?.contains(event.target as Node)) onClose();
    };
    // Deferred so the click that opened the menu does not immediately close it.
    const timer = window.setTimeout(() => document.addEventListener('mousedown', dismiss), 0);
    window.addEventListener('resize', onClose);
    window.addEventListener('scroll', onClose, true);
    return () => {
      window.clearTimeout(timer);
      document.removeEventListener('mousedown', dismiss);
      window.removeEventListener('resize', onClose);
      window.removeEventListener('scroll', onClose, true);
    };
  }, [onClose]);

  return createPortal(
    <div
      className="menu"
      role="menu"
      aria-label={label}
      ref={menuRef}
      style={{ left: position.x, top: position.y }}
      onKeyDown={(event) => {
        if (event.key === 'ArrowDown') {
          event.preventDefault();
          setActiveIndex((index) => (index + 1) % items.length);
        } else if (event.key === 'ArrowUp') {
          event.preventDefault();
          setActiveIndex((index) => (index - 1 + items.length) % items.length);
        } else if (event.key === 'Home') {
          event.preventDefault();
          setActiveIndex(0);
        } else if (event.key === 'End') {
          event.preventDefault();
          setActiveIndex(items.length - 1);
        } else if (event.key === 'Tab') {
          onClose();
        }
      }}
    >
      {items.map((item, index) => (
        <button
          key={item.id}
          type="button"
          role="menuitem"
          tabIndex={index === activeIndex ? 0 : -1}
          className={`menu__item${item.danger ? ' menu__item--danger' : ''}`}
          onClick={() => {
            onClose();
            item.onSelect();
          }}
        >
          {item.icon ? <span aria-hidden="true">{item.icon}</span> : null}
          {item.label}
        </button>
      ))}
    </div>,
    document.body,
  );
}
