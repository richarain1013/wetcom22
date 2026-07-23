using System.Windows;
using WeComLauncher.ViewModels;

namespace WeComLauncher;

public partial class MainWindow : Window
{
    public MainWindow()
    {
        InitializeComponent();
        DataContext = new MainViewModel();
    }
}
