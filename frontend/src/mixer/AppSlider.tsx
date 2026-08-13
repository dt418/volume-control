import { useEffect, useRef, useState } from "react";

import { Slider } from "../components/ui/slider";
import { invoke } from "../lib/ipc";

interface AppSliderProps {
  sessionId: string;
  value: number;
  onError: (error: string) => void;
}

/**
 * Per-session volume slider with the §9.1 echo-jitter guard: optimistic local
 * state + an `isDragging` ref, so backend state refreshes never fight the
 * thumb while the user is dragging. Writes fire without awaiting; failures
 * (e.g. a stale session id) surface through `onError`.
 */
export function AppSlider({ sessionId, value, onError }: AppSliderProps) {
  const [localVal, setLocalVal] = useState(value);
  const isDragging = useRef(false);

  useEffect(() => {
    if (!isDragging.current) setLocalVal(value);
  }, [value]);

  const handleChange = (newVal: number) => {
    setLocalVal(newVal); // immediate UI
    invoke<void>("set_session_volume", { id: sessionId, pct: newVal }).catch(
      (error) => onError(String(error)),
    );
  };

  return (
    <Slider
      value={[localVal]}
      min={0}
      max={100}
      step={1}
      aria-label={`Volume for session ${sessionId}`}
      onPointerDown={() => (isDragging.current = true)}
      onPointerUp={() => (isDragging.current = false)}
      onValueChange={([val]) => handleChange(val)}
    />
  );
}
