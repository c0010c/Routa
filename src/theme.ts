import { createTheme } from "@mui/material/styles";

// Win11-style look built only from MUI theme options. Visual rules live here;
// pages use MUI components and `sx` for layout only.

const numeric = { fontVariantNumeric: "tabular-nums" } as const;

export const theme = createTheme({
  cssVariables: { colorSchemeSelector: "media" },
  colorSchemes: {
    light: {
      palette: {
        primary: { main: "#0067c0" },
        success: { main: "#0f7b0f" },
        warning: { main: "#9d5d00" },
        error: { main: "#c42b1c" },
        background: { default: "#f3f3f3", paper: "#ffffff" },
        divider: "#e5e5e5",
        text: { primary: "#1b1b1b", secondary: "#616161" },
      },
    },
    dark: {
      palette: {
        primary: { main: "#4cc2ff", contrastText: "#000" },
        success: { main: "#6ccb5f" },
        warning: { main: "#fce100" },
        error: { main: "#ff99a4" },
        background: { default: "#202020", paper: "#2b2b2b" },
        divider: "#3a3a3a",
        text: { primary: "#f3f3f3", secondary: "#c5c5c5" },
      },
    },
  },
  shape: { borderRadius: 6 },
  typography: {
    fontFamily: '"Segoe UI Variable Text", "Segoe UI", "Microsoft YaHei UI", "Microsoft YaHei", sans-serif',
    h5: { fontSize: 24, fontWeight: 600 },
    button: { textTransform: "none", fontWeight: 500 },
    body1: numeric,
    body2: numeric,
    caption: numeric,
  },
  components: {
    MuiPaper: {
      defaultProps: { variant: "outlined" },
      styleOverrides: { rounded: { borderRadius: 8 } },
    },
    // Floating surfaces keep their shadow so they read as a layer above the page.
    MuiDialog: {
      defaultProps: { slotProps: { paper: { variant: "elevation", elevation: 8 } } },
      styleOverrides: { paper: { borderRadius: 8 } },
    },
    MuiMenu: { defaultProps: { slotProps: { paper: { variant: "elevation", elevation: 4 } } } },
    MuiPopover: { defaultProps: { slotProps: { paper: { variant: "elevation", elevation: 4 } } } },
    MuiDialogTitle: { styleOverrides: { root: { fontSize: 20, fontWeight: 600 } } },
    MuiButton: {
      defaultProps: { disableElevation: true },
      styleOverrides: {
        root: ({ theme }) => ({
          variants: [
            {
              props: { variant: "outlined", color: "primary" },
              style: { borderColor: theme.vars.palette.divider, color: theme.vars.palette.text.primary },
            },
          ],
        }),
      },
    },
    MuiChip: { defaultProps: { variant: "outlined", size: "small" } },
    MuiTextField: { defaultProps: { size: "small" } },
    MuiTableCell: {
      styleOverrides: {
        root: ({ theme }) => ({ borderColor: theme.vars.palette.divider }),
        head: ({ theme }) => ({ color: theme.vars.palette.text.secondary, fontWeight: 400 }),
      },
    },
    MuiListItemButton: {
      styleOverrides: {
        root: ({ theme }) => ({
          borderRadius: 6,
          position: "relative",
          "&.Mui-selected": {
            backgroundColor: theme.vars.palette.action.selected,
            "& .MuiListItemText-primary": { fontWeight: 600 },
            "&::before": {
              content: '""',
              position: "absolute",
              left: 0,
              top: 10,
              bottom: 10,
              width: 3,
              borderRadius: 3,
              backgroundColor: theme.vars.palette.primary.main,
            },
          },
        }),
      },
    },
  },
});
