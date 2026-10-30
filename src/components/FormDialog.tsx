import Close from "@mui/icons-material/Close";
import { Dialog, IconButton, type DialogProps } from "@mui/material";

/** Drafts live in the caller; every dismissal discards them without a second prompt. */
export function FormDialog({
  onClose,
  children,
  sx,
  ...rest
}: Omit<DialogProps, "open" | "onClose"> & { onClose: () => void }) {
  return (
    <Dialog
      open
      {...rest}
      onClose={onClose}
      sx={[
        {
          "& .MuiDialog-paper": {
            height: 720,
            maxHeight: "calc(100dvh - 40px)",
            m: "20px",
            display: "flex",
            flexDirection: "column",
          },
          "& .MuiDialogTitle-root": { flexShrink: 0, pr: 6 },
          "& .MuiDialogContent-root": {
            overflowY: "auto",
            flex: 1,
            minHeight: 0,
          },
          "& .MuiDialogActions-root": {
            flexShrink: 0,
            p: 2,
            borderTop: 1,
            borderColor: "divider",
          },
          "& .MuiDialogContent-root .MuiStack-root": {
            flexDirection: "column",
          },
          "& .MuiInputLabel-root": {
            position: "static",
            transform: "none",
            mb: 0.75,
            fontSize: 13,
          },
          "& .MuiInputBase-root legend": { display: "none" },
        },
        ...(Array.isArray(sx) ? sx : sx ? [sx] : []),
      ]}
    >
      {children}
      <IconButton
        aria-label="关闭"
        onClick={onClose}
        sx={{ position: "absolute", right: 12, top: 12 }}
      >
        <Close />
      </IconButton>
    </Dialog>
  );
}
