import { useEffect, useState } from "react";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "./ui/dialog";
import { Button } from "./ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "./ui/select";
import { Label } from "./ui/label";
import { Textarea } from "./ui/textarea";

export type FilterMode = "plain" | "jq";

interface MessageFilterModalProps {
  open: boolean;
  onOpenChange: (v: boolean) => void;
  mode: FilterMode;
  value: string;
  onSave: (next: { mode: FilterMode; value: string }) => void;
}

export function MessageFilterModal({ open, onOpenChange, mode, value, onSave }: MessageFilterModalProps) {
  const [localMode, setLocalMode] = useState<FilterMode>(mode);
  const [localValue, setLocalValue] = useState<string>(value || "");

  useEffect(() => {
    if (open) {
      setLocalMode(mode);
      setLocalValue(value || "");
    }
  }, [open, mode, value]);

  const handleSave = () => {
    onSave({ mode: localMode, value: localValue });
    onOpenChange(false);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[42rem]">
        <DialogHeader>
          <DialogTitle>Message Filter</DialogTitle>
        </DialogHeader>
        <div className="space-y-4">
          <div>
            <Label>Filter Mode</Label>
            <Select value={localMode} onValueChange={(v) => setLocalMode(v as FilterMode)}>
              <SelectTrigger>
                <SelectValue placeholder="Select mode" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="plain">Plain contains</SelectItem>
                <SelectItem value="jq">JQ (JSON)</SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div>
            <Label>Expression</Label>
            <Textarea
              placeholder={localMode === "jq" ? ".foo == \"bar\" or .a.b[0] == 1" : "Type part of message to search for"}
              value={localValue}
              onChange={(e) => setLocalValue(e.target.value)}
              className="min-h-[160px]"
            />
            {localMode === "jq" && (
              <p className="text-xs text-muted-foreground mt-2">
                JQ filter must evaluate to boolean true. Example: <code className="font-mono">.user.id == 42</code>.
                Protobuf messages are decoded to compact JSON before filtering.
              </p>
            )}
          </div>
          <div className="flex justify-end gap-2 pt-2">
            <Button variant="outline" onClick={() => onOpenChange(false)}>Cancel</Button>
            <Button onClick={handleSave}>Save</Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
