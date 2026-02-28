import { useState, useEffect, useRef } from "react";

/**
 * 追踪滚动容器内哪条用户消息当前可见。
 * 当多条可见时，取最靠近容器顶部 45% 处的那条。
 */
export function useActiveUserMessage(
  containerRef: React.RefObject<HTMLDivElement | null>,
  userMessageIds: string[]
): string | null {
  const [activeId, setActiveId] = useState<string | null>(null);
  const observerRef = useRef<IntersectionObserver | null>(null);

  useEffect(() => {
    const container = containerRef.current;
    if (!container || userMessageIds.length === 0) {
      setActiveId(null);
      return;
    }

    observerRef.current?.disconnect();

    const visibleEntries = new Map<string, IntersectionObserverEntry>();

    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          const id = (entry.target as HTMLElement).dataset.userMsgId;
          if (!id) continue;
          if (entry.isIntersecting) {
            visibleEntries.set(id, entry);
          } else {
            visibleEntries.delete(id);
          }
        }

        if (visibleEntries.size > 0) {
          const containerRect = container.getBoundingClientRect();
          const targetY = containerRect.top + containerRect.height * 0.45;
          let bestId = "";
          let bestDist = Infinity;
          for (const [id, entry] of visibleEntries) {
            const dist = Math.abs(entry.boundingClientRect.top - targetY);
            if (dist < bestDist) {
              bestDist = dist;
              bestId = id;
            }
          }
          setActiveId(bestId);
        }
      },
      {
        root: container,
        rootMargin: "0px",
        threshold: 0.1,
      }
    );

    observerRef.current = observer;

    for (const id of userMessageIds) {
      const el = container.querySelector(`[data-user-msg-id="${id}"]`);
      if (el) observer.observe(el);
    }

    return () => observer.disconnect();
  }, [containerRef, userMessageIds]);

  return activeId;
}
