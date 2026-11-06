import ArrowBackOutlined from "@mui/icons-material/ArrowBackOutlined";
import { IconButton, Stack, Typography } from "@mui/material";
import { Link } from "react-router-dom";
export function BackHeading({ title }: { title: string }) {
  return (
    <Stack direction="row" spacing={1.5} sx={{ mb: 3, alignItems: "center" }}>
      <IconButton component={Link} to="/" aria-label="返回连接">
        <ArrowBackOutlined />
      </IconButton>
      <Typography variant="h5">{title}</Typography>
    </Stack>
  );
}
