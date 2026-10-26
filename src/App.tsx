import {
  Box,
  CssBaseline,
  Drawer,
  List,
  ListItemButton,
  ListItemText,
  Typography,
} from "@mui/material";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { HashRouter, NavLink, Route, Routes } from "react-router-dom";

const DRAWER_WIDTH = 200;

const pages = [
  { path: "/", label: "概览" },
  { path: "/listeners", label: "Listener" },
  { path: "/exits", label: "出口" },
  { path: "/nodes", label: "节点" },
  { path: "/settings", label: "设置" },
];

const queryClient = new QueryClient();

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <HashRouter>
        <CssBaseline />
        <Box sx={{ display: "flex" }}>
          <Drawer
            variant="permanent"
            sx={{
              width: DRAWER_WIDTH,
              flexShrink: 0,
              "& .MuiDrawer-paper": { width: DRAWER_WIDTH, boxSizing: "border-box" },
            }}
          >
            <List component="nav" aria-label="主导航">
              {pages.map((p) => (
                <ListItemButton key={p.path} component={NavLink} to={p.path} end={p.path === "/"}>
                  <ListItemText primary={p.label} />
                </ListItemButton>
              ))}
            </List>
          </Drawer>
          <Box component="main" sx={{ flexGrow: 1, p: 3 }}>
            <Routes>
              {pages.map((p) => (
                <Route
                  key={p.path}
                  path={p.path}
                  element={<Typography variant="h5">{p.label}</Typography>}
                />
              ))}
            </Routes>
          </Box>
        </Box>
      </HashRouter>
    </QueryClientProvider>
  );
}
