import Card from '@mui/material/Card';
import CardContent from '@mui/material/CardContent';
import Divider from '@mui/material/Divider';
import List from '@mui/material/List';
import ListItem from '@mui/material/ListItem';
import ListItemText from '@mui/material/ListItemText';
import Typography from '@mui/material/Typography';
import { useLanguage } from '../../i18n/index.js';

export function ActivityList({ activities }) {
  const { t } = useLanguage();

  return (
    <Card sx={{ height: '100%' }}>
      <CardContent>
        <Typography variant="h6" gutterBottom>
          {t('dashboard.activities.title')}
        </Typography>
        <List>
          {activities.map((activity, index) => (
            <div key={activity.id}>
              <ListItem>
                <ListItemText
                  primary={activity.title}
                  secondary={
                    <>
                      <Typography component="span" variant="body2" color="text.primary">
                        {activity.description}
                      </Typography>
                      <br />
                      <Typography component="span" variant="caption" color="text.secondary">
                        {activity.time}
                      </Typography>
                    </>
                  }
                />
              </ListItem>
              {index < activities.length - 1 && <Divider />}
            </div>
          ))}
        </List>
      </CardContent>
    </Card>
  );
}
