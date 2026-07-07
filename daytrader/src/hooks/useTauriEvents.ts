import { useEffect, useRef } from 'react';
import { listen, Event as TauriEvent } from '@tauri-apps/api/event';

/**
 * Subscribes to a Tauri backend event for the lifetime of the calling
 * component, automatically unlistening on unmount or when `event` changes.
 *
 * `handler` is captured in a ref so callers can pass an inline closure
 * without re-subscribing on every render — only changes to `event` itself
 * re-register the listener.
 *
 * @example
 * ```tsx
 * useTauriEvents<RiskViolationEventPayload>('risk:violation', (payload) => {
 *   console.log('violation', payload);
 * });
 * ```
 */
export function useTauriEvents<T>(
  event: string,
  handler: (payload: T, raw: TauriEvent<T>) => void
): void {
  const handlerRef = useRef(handler);
  handlerRef.current = handler;

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | null = null;

    listen<T>(event, (raw) => {
      handlerRef.current(raw.payload, raw);
    }).then((fn) => {
      if (cancelled) {
        // Component unmounted (or `event` changed) before the listener
        // registration resolved — tear it down immediately.
        fn();
        return;
      }
      unlisten = fn;
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [event]);
}
