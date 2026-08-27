import { useCallback, type KeyboardEvent } from "react";

export interface ListNavigation {
  index: number;
  onKeyDown: (event: KeyboardEvent<HTMLElement>) => void;
  itemProps: (position: number) => {
    tabIndex: number;
    "data-selected": boolean;
    onFocus: () => void;
  };
}

/** Roving tabindex over a vertical list: arrows move, Home/End jump, Enter activates. */
export function useListNavigation(
  count: number,
  index: number,
  setIndex: (next: number) => void,
  onActivate?: (position: number) => void,
): ListNavigation {
  const onKeyDown = useCallback(
    (event: KeyboardEvent<HTMLElement>) => {
      if (count === 0) return;
      let next = index;
      if (event.key === "ArrowDown") next = Math.min(count - 1, index + 1);
      else if (event.key === "ArrowUp") next = Math.max(0, index - 1);
      else if (event.key === "Home") next = 0;
      else if (event.key === "End") next = count - 1;
      else if ((event.key === "Enter" || event.key === " ") && onActivate) {
        event.preventDefault();
        onActivate(index);
        return;
      } else return;
      event.preventDefault();
      setIndex(next);
      const container = event.currentTarget;
      const items = container.querySelectorAll<HTMLElement>("[data-nav-item]");
      items[next]?.focus();
    },
    [count, index, onActivate, setIndex],
  );

  const itemProps = useCallback(
    (position: number) => ({
      tabIndex: position === index ? 0 : -1,
      "data-selected": position === index,
      onFocus: () => setIndex(position),
    }),
    [index, setIndex],
  );

  return { index, onKeyDown, itemProps };
}
