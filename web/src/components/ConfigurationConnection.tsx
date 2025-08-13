import { useLayoutEffect, useRef, useState } from "react";
import { Card, CardContent } from "./ui/card";
import { Label } from "./ui/label";
import { Input } from "./ui/input";
import { createPortal } from "react-dom";

// Minimal shape needed for this component
interface ConfigShape {
  broker: string;
  topic: string;
}

interface Props {
  config: ConfigShape;
  setConfig: React.Dispatch<React.SetStateAction<any>>;
  isFetchingTopics: boolean;
  topicsError: string | null;
  filteredTopics: string[];
  handleBrokerBlur: () => void;
  handleBrokerChange: (val: string) => void;
}

export default function ConfigurationConnection({
  config,
  setConfig,
  isFetchingTopics,
  topicsError,
  filteredTopics,
  handleBrokerBlur,
  handleBrokerChange,
}: Props) {
  const [topicFocused, setTopicFocused] = useState(false);
  const [topicDropdownPos, setTopicDropdownPos] = useState<{ left: number; top: number; width: number } | null>(null);
  const [topicDropdownHover, setTopicDropdownHover] = useState(false);
  const [topicDropdownInteracting, setTopicDropdownInteracting] = useState(false);

  const dropdownRef = useRef<HTMLDivElement | null>(null);

  const updateTopicDropdownPos = () => {
    const el = document.getElementById('topic');
    if (!el) { setTopicDropdownPos(null); return; }
    const rect = el.getBoundingClientRect();
    setTopicDropdownPos({ left: Math.round(rect.left), top: Math.round(rect.bottom + 2), width: Math.round(rect.width) });
  };

  useLayoutEffect(() => {
    if (!topicFocused) return;
    updateTopicDropdownPos();

    const onResize = () => updateTopicDropdownPos();
    const onScroll = (e: Event) => {
      const target = e.target as Node | null;
      const dr = dropdownRef.current;
      if (dr && target && (target === dr || dr.contains(target))) {
        return;
      }
      updateTopicDropdownPos();
    };

    window.addEventListener('resize', onResize);
    document.addEventListener('scroll', onScroll, true);

    const ro = new ResizeObserver(() => updateTopicDropdownPos());
    const el = document.getElementById('topic');
    if (el) ro.observe(el);

    return () => {
      window.removeEventListener('resize', onResize);
      document.removeEventListener('scroll', onScroll, true);
      ro.disconnect();
    };
  }, [topicFocused]);

  return (
    <Card>
      <CardContent className="pt-2 space-y-4">
        <div>
          <Label htmlFor="broker">Kafka Broker</Label>
          <Input
            id="broker"
            value={config.broker}
            onChange={(e) => {
              const val = e.target.value;
              handleBrokerChange(val);
            }}
            onBlur={handleBrokerBlur}
            placeholder="localhost:9092"
          />
        </div>
        <div className="relative">
          <Label htmlFor="topic">Topic</Label>
          <Input
            id="topic"
            value={config.topic}
            onFocus={() => setTopicFocused(true)}
            onBlur={() => setTimeout(() => {
              if (!topicDropdownHover && !topicDropdownInteracting) setTopicFocused(false);
            }, 250)}
            onChange={(e) => setConfig((prev: any) => ({ ...prev, topic: e.target.value }))}
            placeholder="my-topic"
            autoComplete="off"
          />
          {(topicFocused && (isFetchingTopics || topicsError || filteredTopics.length > 0)) && topicDropdownPos && (
            createPortal(
              <div
                ref={dropdownRef}
                onMouseEnter={() => setTopicDropdownHover(true)}
                onMouseLeave={() => setTopicDropdownHover(false)}
                onMouseDown={() => setTopicDropdownInteracting(true)}
                onMouseUp={() => setTimeout(() => setTopicDropdownInteracting(false), 0)}
                onPointerDown={() => setTopicDropdownInteracting(true)}
                onPointerUp={() => setTimeout(() => setTopicDropdownInteracting(false), 0)}
                onWheelCapture={(e) => { e.stopPropagation(); }}
                onWheel={(e) => { e.stopPropagation(); }}
                onTouchMove={(e) => { e.stopPropagation(); }}
                style={{ position: 'fixed', left: topicDropdownPos.left, top: topicDropdownPos.top, width: topicDropdownPos.width, zIndex: 1000, maxHeight: 240, overflowY: 'auto', pointerEvents: 'auto', overscrollBehavior: 'contain', WebkitOverflowScrolling: 'touch' as any, touchAction: 'pan-y' as any }}
                className="rounded-md border bg-background shadow-md"
              >
                {isFetchingTopics && (
                  <div className="px-3 py-2 text-sm text-muted-foreground">Loading topics…</div>
                )}
                {!isFetchingTopics && topicsError && (
                  <div className="px-3 py-2 text-sm text-destructive">{topicsError}</div>
                )}
                {!isFetchingTopics && !topicsError && filteredTopics.map((t) => (
                  <div
                    key={t}
                    className="px-3 py-2 cursor-pointer hover:bg-accent hover:text-accent-foreground text-sm"
                    onMouseDown={() => {
                      setConfig((prev: any) => ({ ...prev, topic: t }));
                      setTopicFocused(false);
                    }}
                  >
                    {t}
                  </div>
                ))}
                {!isFetchingTopics && !topicsError && filteredTopics.length === 0 && (
                  <div className="px-3 py-2 text-sm text-muted-foreground">No matching topics</div>
                )}
              </div>,
              document.body
            )
          )}
        </div>
      </CardContent>
    </Card>
  );
}
