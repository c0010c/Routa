import { Button, Dialog, DialogActions, DialogContent, DialogTitle, type DialogProps } from "@mui/material";
import { useState, type SyntheticEvent } from "react";

// Clicking blank space or the backdrop is not an edit; touching a control is.
const CONTROLS = "input, textarea, button, label, [role=option], [role=combobox], [role=button], [role=checkbox], [role=switch]";

/**
 * A dialog holding a form. Esc / clicking outside closes it right away while untouched; once the user
 * has typed or clicked a control inside, it asks before throwing the input away. 取消 buttons call
 * `onClose` directly and are not affected.
 */
export function FormDialog({
  onClose,
  children,
  ...rest
}: Omit<DialogProps, "open" | "onClose"> & { onClose: () => void }) {
  const [touched, setTouched] = useState(false);
  const [confirming, setConfirming] = useState(false);
  // React events bubble through portals, so this also sees Select menus and Autocomplete popups.
  const touch = (e: SyntheticEvent) => {
    if ((e.target as HTMLElement).closest?.(CONTROLS)) setTouched(true);
  };
  return (
    <Dialog
      open
      {...rest}
      onInputCapture={touch}
      onClickCapture={touch}
      onClose={() => (touched ? setConfirming(true) : onClose())}
    >
      {children}
      {confirming && (
        <Dialog open onClose={() => setConfirming(false)} maxWidth="xs">
          <DialogTitle>放弃未保存的修改？</DialogTitle>
          <DialogContent>关闭后，刚才填写的内容不会保存。</DialogContent>
          <DialogActions>
            <Button onClick={() => setConfirming(false)}>继续编辑</Button>
            <Button color="error" onClick={onClose}>
              放弃
            </Button>
          </DialogActions>
        </Dialog>
      )}
    </Dialog>
  );
}
