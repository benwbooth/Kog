/**
 * tabs.m
 *
 * Attention: the ID values for internal tabs are saved in studio.xnf incremented by one!
 *
 * @author mpdeimos
 * @date 08/11/11
 * @version 0.1
 */

#include <lib/std.mi>
#include <lib/config.mi>
#include <lib/colormgr.mi>
Global ColorMgr StartupCallback;

#define DISPATCH
#include ../../../../scripts/dispatch_codes.m

//#define DEBUG
#define debugTabs //
#define QUE_MAN "{F8CD22A4-0C33-45FB-A360-70D7240FC23E}"

// this is the page that maps its items to the windows menu (aka View), you can add attribs or more pages (submenus)
#define CUSTOM_WINDOWSMENU_ITEMS "{6559CA61-7EB2-4415-A8A9-A2AEEF762B7F}"

Class GuiObject Tab;
// {
	Member Int Tab.mid;
	Member Int Tab.w;
	Member Int Tab.maxW;
	Member Int Tab.lastX;
	Member Int Tab.initX;
	Member Int Tab.pos;
	Member Boolean Tab.moving;
	Member Boolean Tab.isInternal;
	Member Int Tab.ID;
	Member String Tab.IDS;
	Member String Tab.nameLong;
	Member String Tab.nameShort;
	Member String Tab.iconbitmap;
	Member boolean Tab.removed;
	Member boolean Tab.statusbar;

	//Stimulating a doubled linked list
	Member GuiObject Tab.left;
	Member GuiObject Tab.right;
// }

#define WIDGET_TAB_ID -666

Function moveLeft (Tab t);
Function moveRight (Tab t);
Function moveTo (Tab g, int x);
Function updateTabWidth(Tab t);
Function forceAlign(Tab t);
Function align(Tab t); // uses one of the two methods below
Function alignFull(Tab t);
Function alignByResize();
Function removeTab(Tab t);
Function closeTab(Tab t);
Function updateTabCount();

#ifdef DEBUG
Function debugTab (Tab t);
Function debugTabs ();
#endif

Global Tab firstTab, lastTab, lastActiveT;
Global ToggleButton lastActive;
Global Group sg, tabHolder, CproSUI, CproBrowser;
Global int totalTabWidth, widthMouseDown, tabCount;
Global ComponentBucket widgetLoader;
Global boolean aligned, checkedBrowser, isFullNames; // @martin: remove this when done ;)
Global PopUpMenu popMenu;
Global List hiddenTabs;
Global Text t_temp;
Global Boolean more_icons;

Global ConfigItem custom_windows_page;
Class ConfigAttribute CproTabAtt;
	Member Int CproTabAtt.ID;
	Member String CproTabAtt.IDS;

Global CproTabAtt tabWinAtt;
Global ConfigAttribute sui_browser_attrib, sui_eq_attrib, sui_vis_attrib;

Global Boolean loaded;

/*Global Container player;
Global Layout normal;*/

System.onScriptLoaded ()
{
	StartupCallback = new ColorMgr;

	checkedBrowser = false;
	
	sg = getScriptGroup();
	tabHolder = sg.findObject("cprotabs.buttons");
	widgetLoader = sg.findObject("widget.loader");

	setDispatcher(tabHolder);

	CproSUI = getScriptGroup().getParent().getParent().getParent().getParent();
	
	Map m = new Map;
	m.loadMap("cpro.tab.icon.ple.1");
	if(m.getHeight()==25) more_icons=true;
	delete m;

	/*player = getContainer("main");
	normal = player.getLayout("normal");*/

	/*
	ALL THE STUFF THATS LOADED ONSETVISIBLE WAS HERE!!!!!!!!!
	main reason for this move was so that tab sizes dont need to update twice if skinner change text style
	*/

}

System.onScriptUnloading ()
{
	if(!loaded) return;
	
	/** save tab order */
	Tab t = firstTab;
	Int n;
	while (t != NULL)
	{
		if (t.isInternal)
		{
			setPrivateInt("ClassicPro", "TabOrder_item_" + integerToString(n), t.ID+1);
		}
		else
		{
			setPrivateString("ClassicPro", "TabOrder_item_" + integerToString(n), t.IDS);
		}
		
		n++;
		t = t.right;
	}

	setPrivateInt("ClassicPro", "TabOrder_nItems", n);

	/** save hidden tabs (we just concat the lists for now!)*/
	//TODO move to extra list in studio.xnf

	//n = 0;

	for ( int i = 0; i < hiddenTabs.getNumItems(); i++ )
	{
		t = hiddenTabs.enumItem(i);
		/*if (t.isInternal)
		{
			setPrivateInt("ClassicPro", "TabOrder_hiddenItem_" + integerToString(n), t.ID+1);
		}
		else
		{
			setPrivateString("ClassicPro", "TabOrder_hiddenItem_" + integerToString(n), t.IDS);
		}*/
		if (t.isInternal)
		{
			setPrivateInt("ClassicPro", "TabOrder_item_" + integerToString(n), t.ID+1);
		}
		else
		{
			setPrivateString("ClassicPro", "TabOrder_item_" + integerToString(n), t.IDS);
		}
		
		n++;
	}

	//setPrivateInt("ClassicPro", "TabOrder_nHiddenItems", n);
	setPrivateInt("ClassicPro", "TabOrder_nItems", n);

	delete hiddenTabs;
}

sg.onSetVisible(boolean onOff){
	if(onOff && !loaded){
	loaded=true;




	//TODO> use stringtables
	List internalNames = new List;
	internalNames.addItem("Media Library");		//0
	internalNames.addItem("Playlist");			//1
	internalNames.addItem("Video");				//2
	internalNames.addItem("Visualization");		//3
	internalNames.addItem("Web Reader");			//4
	//internalNames.addItem("Queue Manager");		//5
	internalNames.addItem("Plugin");			//5

	List internalTabIcons = new List;
	
	/*String icon_suffix;
	if(more_icons) icon_suffix = ".";
	else icon_suffix ="";*/
	
	
	internalTabIcons.addItem("cpro.tab.icon.lib");		//0
	internalTabIcons.addItem("cpro.tab.icon.ple");		//1
	internalTabIcons.addItem("cpro.tab.icon.vid");		//2
	internalTabIcons.addItem("cpro.tab.icon.vis");		//3
	internalTabIcons.addItem("cpro.tab.icon.bro");		//4
	//internalTabIcons.addItem("cpro.tab.icon.que");	//5
	internalTabIcons.addItem("cpro.tab.icon.plu");		//5
	
	/*Map m = new Map;
	m.loadMap("cpro.tab.icon.ple.3");
	if(m.getHeight()==25) moreIcons=true;
	delete m;*/

	/** Create ordered list of all saved tabs */

	Bitlist isInternal = new BitList;
	hiddenTabs = new List;
	List orderedTabs = new List;
	List widgetNames = new List;
	//List widgetNames2 = new List;
	List widgetTabIcons = new List;
	Bitlist passedWidgets = new BitList;
	Bitlist skipWidget = new BitList;
	passedWidgets.setSize(widgetLoader.getNumChildren());
	// TODO maybe remove some lists :P

	int n = getPrivateInt("ClassicPro", "TabOrder_nItems", 0);
	skipWidget.setSize(n);
	
	isFullNames = true;
	
	
	if (n == 0) // First start, we need to init!
	{
		isInternal.setSize(internalNames.getNumItems());
		for ( int i = 0; i < internalNames.getNumItems(); i++ )
		{
			isInternal.setItem(i, true);
			orderedTabs.addItem(i);
		}

		for (int i = 0; i < widgetLoader.getNumChildren(); i++)
		{
			isInternal.setItem(internalNames.getNumItems()+i, false);
			GuiObject d = widgetLoader.enumChildren(i);
			orderedTabs.addItem(d.getXmlParam("userdata"));
			widgetNames.addItem(getToken(d.getXmlParam("name"), ";", 0));
			//widgetNames2.addItem(getToken(d.getXmlParam("name"), ";", 1));
			widgetTabIcons.addItem(getToken(d.getXmlParam("name"), ";", 1));
		}
	}
	else
	{
		isInternal.setSize(n);
		for ( int i = 0; i < n; i++ )
		{
			String ids = getPrivateString("ClassicPro", "TabOrder_item_" + integerToString(i), "");
			int id = stringToInteger(ids);
			if (id)
			{
				isInternal.setItem(i, true);
				orderedTabs.addItem(id-1);
			}
			else
			{
				// IMPORTANT! We need to check if the widget has been removed
				if (!isInternal.getItem(i))
				{
					boolean found = false;
					for ( int j = 0; j < widgetLoader.getNumChildren(); j++ )
					{
						GuiObject d = widgetLoader.enumChildren(j);
						if (getToken(d.getXmlParam("userdata"), ";", 0) == ids)
						{
							passedWidgets.setItem(j, true); // Mark this widget to be inited
							widgetNames.addItem(getToken(d.getXmlParam("name"), ";", 0));
							//widgetNames2.addItem(getToken(d.getXmlParam("name"), ";", 1));
							widgetTabIcons.addItem(getToken(d.getXmlParam("name"), ";", 1));
							isInternal.setItem(i, false);
							orderedTabs.addItem(ids);
							found = true;
							break;
						}
					}
					if (!found)// this means the widget must have been deleted!
					{
						skipWidget.setItem(i, true);
						orderedTabs.addItem(NULL);
					}
				}
			}
		}

		/** One last thing to do is that there might have been new widgets installed since last engine load */
		for ( int i = 0; i < passedWidgets.getSize(); i++ )
		{
			if (!passedWidgets.getItem(i))
			{
				isInternal.setSize(isInternal.getSize()+1);
				isInternal.setItem(isInternal.getSize()-1, false);
				GuiObject d = widgetLoader.enumChildren(i);
				orderedTabs.addItem(d.getXmlParam("userdata"));
				widgetNames.addItem(getToken(d.getXmlParam("name"), ";", 0));
				//widgetNames2.addItem(getToken(d.getXmlParam("name"), ";", 1));
				widgetTabIcons.addItem(getToken(d.getXmlParam("name"), ";", 2));
			}
		}
	}


	/** Load Window menu */
	custom_windows_page = Config.getItem(CUSTOM_WINDOWSMENU_ITEMS);
	sui_vis_attrib = custom_windows_page.newAttribute("Visualizations\tCtrl+Shift+K", "0");
	sui_eq_attrib = custom_windows_page.newAttribute("Equalizer\tAlt+G", "0");
	sui_browser_attrib = custom_windows_page.newAttribute("Web Reader\tAlt+X", "0");
	
	/** Bring ordered tabs into action */

	int initPos = 0;
	GuiObject pre = NULL;

	for ( int i = 0; i < orderedTabs.getNumItems(); i++ )
	{
		if (!skipWidget.getItem(i))
		{			
			Tab tabI = newGroup("cpro.tab");
			tabI.setXmlParam("x", integerToString(totalTabWidth));
			tabI.setXmlParam("y", "0");

			if (isInternal.getItem(i)) // internal tab?
			{
				tabI.ID = orderedTabs.enumItem(i);
				tabI.IDS = "";
				tabI.isInternal = true;
				tabI.nameLong = internalNames.enumItem(tabI.ID);
				//debug(internalNames.enumItem(tabI.ID));
				//tabI.nameShort = internalNamesShort.enumItem(tabI.ID);
				tabI.iconbitmap = internalTabIcons.enumItem(tabI.ID);
			}
			else
			{
				tabWinAtt = custom_windows_page.newAttribute(widgetNames.enumItem(0), "0");
				tabWinAtt.IDS = getToken(orderedTabs.enumItem(i), ";", 0);

				tabI.ID = WIDGET_TAB_ID;
				tabI.IDS = getToken(orderedTabs.enumItem(i), ";", 0);
				tabI.statusbar = stringToInteger(getToken(orderedTabs.enumItem(i), ";", 1));
				tabI.isInternal = false;
				tabI.nameLong = widgetNames.enumItem(0);
				tabI.iconbitmap = widgetTabIcons.enumItem(0);
				
				//widgetTabIcons.addItem(getToken(d.getXmlParam("name"), ";", 2));
				
				//if(widgetNames2.enumItem(0)!="") tabI.nameShort = strupper(strLeft(widgetNames2.enumItem(0),3)); //Force all languagepacks to 3max & uppercase
				//else tabI.nameShort = strupper(strLeft(widgetNames.enumItem(0),3));
				
				//debugstring(integerToString(widgetNames.getNumItems())+" _ "+integerToString(widgetNames2.getNumItems()) + widgetNames2.enumItem(0),9);
				widgetNames.removeItem(0);
				//widgetNames2.removeItem(0);
				widgetTabIcons.removeItem(0);
			}

			Boolean hideTab;
			if(tabI.ID==WIDGET_TAB_ID){
				hideTab = ((getPublicInt("cpro2.One.TabAutoClose."+tabI.IDS, 0)) && !(tabI.IDS==getPublicString("cpro2.lastMainWidgetIDS", "") && getPublicInt("cpro2.lastComponentPage", 0)==WIDGET_TAB_ID));
			}
			else hideTab = ((getPublicInt("cpro2.One.TabAutoClose."+integerToString(tabI.ID), 0) || tabI.ID == 5) && (tabI.ID != getPublicInt("cpro2.lastComponentPage", 0)));

			if (!hideTab)
			{
				tabI.pos = i;
				tabI.left = pre;

				if (pre != NULL)
				{
					Tab l = pre;
					l.right = tabI;	
				}
				pre = tabI;
				tabI.right = NULL;
			}
			else
			{
				tabI.setXmlParam("visible", "0");
				hiddenTabs.addItem(tabI);
				tabI.removed = true;
			}
			

			tabI.init(tabHolder);

			Text t = tabI.findObject("cpro.tab.text");
			t.setXmlParam("text", tabI.nameLong);

			Layer l = tabI.findObject("cpro.tab.icon");
			l.setXmlParam("tooltip", tabI.iconbitmap);
			if(more_icons) l.setXmlParam("image", tabI.iconbitmap+".1");
			else l.setXmlParam("image", tabI.iconbitmap);
			
			ToggleButton t2 = tabI.findObject("cpro.tab.button");
			t2.setXmlParam("tooltip", tabI.nameLong);
			
			//t.setXmlParam("text", tabI.nameShort);
			
			
			/*if (tabI.isInternal)
			{
				t.setXmlParam("text", tabI.nameLong);
				//t.setXmlParam("text", internalNames.enumItem(tabI.ID));	
			}
			else
			{
				//t.setXmlParam("text", widgetNames.enumItem(0));
				//widgetNames.removeItem(0);
			}*/
			
			updateTabWidth(tabI);
			if (hideTab)
			{
				totalTabWidth -= tabI.w;				
			}

		}
	}

	aligned = true;
	lastTab = pre;
	Tab t = pre;

	while (t.left != null)
	{
		t = t.left;
	}
	
	lastActive = lastActiveT = NULL;
	firstTab = t;

	debugTabs();

	delete internalNames;
	delete internalTabIcons;
	delete isInternal;
	delete widgetNames;
	//delete widgetNames2;
	delete widgetTabIcons;
	delete skipWidget;
	delete passedWidgets;
	
	tabHolder.onResize (1,1,1,1);
	}
}


tabHolder.onResize (int x, int y, int w, int h)
{
	if (w <= 0)
	{
		return;
	}
	
	align(firstTab);
}

onMessage(int message, int i0, int i1, int i2, String s0, String s1, GuiObject obj)
{
	Tab t = obj;

	if (message == ON_LEFT_BUTTON_DOWN)
	{
		widthMouseDown = tabHolder.getWidth();
		
		if (t.isGoingToTarget())
			return SUCCESS;

		t.lastX = i0;
		t.initX = t.getGuiX();
		t.bringToFront();
		t.moving = true;

		return SUCCESS;
	}
	if (message == ON_LEFT_BUTTON_UP)
	{
		if (t.isGoingToTarget())
			return SUCCESS;

		//Fixed bug where if the side playlist was open and you click the pl tab the pl tab button will move to the middle of nowwhere when click is done.
		if(widthMouseDown == tabHolder.getWidth()){ 
			if (t.moving || i2)
			{
				moveTo(t, t.initX);
				debugTabs();
			}
		}
		
		t.moving = false;

		t.hide();
		t.show(); //so we update hover state

		return SUCCESS;
	}
	else if (message == ON_MOUSE_MOVE)
	{
		if (t.isGoingToTarget())
			return SUCCESS;

		if (t.moving)
		{
			t.bringToFront();
			int newPos = i0 - t.lastX + t.getGuiX();

			if (newPos < 0)
			{
				newPos = 0;
			}
			else if (newPos > tabHolder.getWidth() - t.w)
			{
				newPos = tabHolder.getWidth() - t.w;
			}
			else
			{
				t.lastX = i0;	
			}

			t.setXmlParam("x", integerToString(newPos));

			Tab left = t.left;
			if (left != null)
			{
				if (newPos < left.mid + left.getGuiX() && !left.isGoingToTarget())
				{
					moveLeft(t);
					return SUCCESS;
				}
			}
			Tab right = t.right;
			if (right != null)
			{
				if (newPos > right.mid + right.getGuiX() - t.getGuiW() && !right.isGoingToTarget())
				{
					moveRight(t);
				}
			}

			return SUCCESS;

		}
	}
	else if (message == ON_TAB_ACTIVATED)
	{
		closeTab(lastActiveT);
		CproSUI.sendAction ("show_tab", t.IDS, t.ID, 0, 0, 0);
		
		if(t.ID==WIDGET_TAB_ID){
			CproSUI.sendAction ("widget_statusbar", "", t.statusbar, 0, 0, 0);
		}
		lastActive = lastActiveT = t;
		t.moving = false;
	}
	else if (message == ON_LEFT_DBL_CLICK)
	{
		CproSUI.sendAction ("toggle_fs", "", 0, 0, 0, 0);
	
	}
	else if (message == ON_RIGHT_BUTTON_UP)
	{
		int tabID = t.ID;
		String widID = t.IDS;
		popMenu = new PopUpMenu;
		
		if(tabID==0 || tabID==4 || tabID==5 || tabID==WIDGET_TAB_ID) popMenu.addCommand("Show Status Bar", 0, 0, 1);
		else popMenu.addCommand("Show Status Bar", 0, getPublicInt("cpro2.One.TabStatus."+integerToString(tabID), 1), 0);
		
		if(tabID==5) popMenu.addCommand("Auto Close Tab", 1, 1, 1);
		else if(tabID==WIDGET_TAB_ID) popMenu.addCommand("Auto Close Tab", 1, getPublicInt("cpro2.One.TabAutoClose."+widID, 0), 0);
		else popMenu.addCommand("Auto Close Tab", 1, getPublicInt("cpro2.One.TabAutoClose."+integerToString(tabID), 0), 0);
		// @martin here is the publicint set.. hide tab if int == 1... remember widgets cant hide... 
		//not going to add the previous submenu where you can enable things again..
		// must just add Web Browser to the View list in winamp now so that when its hidden the user can still open it via that  menu

		// The MediaLibrary tab must hide when its not installed (v1.04 already had this).. 
		//but forgot to not use MLIB as the default fallback tab when a tab close :P... will add this before 1.1 in centrosui

#ifdef DEBUG

		popMenu.addSeparator();

		if (t.left != NULL)
		{
			Tab d = t.left;
			popMenu.addCommand("LeftTab: " + d.findObject("cpro.tab.text").getXmlParam("text"), -1, 0, 1);
		}
		else
		{
			popMenu.addCommand("LeftTab: " + "~", -1, 0, 1);
		}

		if (t.right != NULL)
		{
			Tab d = t.right;
			popMenu.addCommand("RightTab: " + d.findObject("cpro.tab.text").getXmlParam("text"), -1, 0, 1);
		}
		else
		{
			popMenu.addCommand("RightTab: " + "~", -1, 0, 1);
		}

		popMenu.addSeparator();

		Tab d = firstTab;
		popMenu.addCommand("FirstTab: " + d.findObject("cpro.tab.text").getXmlParam("text"), -1, 0, 1);
		d = lastTab;
		popMenu.addCommand("LastTab: " + d.findObject("cpro.tab.text").getXmlParam("text"), -1, 0, 1);

		popMenu.addSeparator();

		for ( int i = 0; i < hiddenTabs.getNumItems(); i++ )
		{
			d = hiddenTabs.enumItem(i);
			popMenu.addCommand("Hidden: " + d.findObject("cpro.tab.text").getXmlParam("text"), -1, 0, 1);
		}
		

#endif
				
		int result = popMenu.popAtXY(i1,i2);
		delete popMenu;
		
		if(result!=-1)
		{
			if(result == 0)
			{
				setPublicInt("cpro2.One.TabStatus."+integerToString(tabID),!getPublicInt("cpro2.One.TabStatus."+integerToString(tabID), 1));
				CproSUI.sendAction ("refresh_tab_status", "", 0, 0, 0, 0);
			}
			else if(result == 1)
			{
				if(tabID==WIDGET_TAB_ID) setPublicInt("cpro2.One.TabAutoClose."+widID, !getPublicInt("cpro2.One.TabAutoClose."+widID, 0));
				else setPublicInt("cpro2.One.TabAutoClose."+integerToString(tabID), !getPublicInt("cpro2.One.TabAutoClose."+integerToString(tabID), 0));
				
				ToggleButton tg = t;
	
				if(!tg.getActivated()){ 
					if((tabID==WIDGET_TAB_ID && getPublicInt("cpro2.One.TabAutoClose."+widID, 0)) || (tabID!=WIDGET_TAB_ID && getPublicInt("cpro2.One.TabAutoClose."+integerToString(tabID), 0))){
						removeTab(t);
					}
				}
			}
		}
		complete;
	}
		
	// TODO
	/*else if (message == SET_TAB_W)
	{
		updateTabWidth (t);
		
		return SUCCESS;
	}*/
}

moveLeft (Tab t)
{
	Tab left = t.left;
	Tab left2 = left.left;
	Tab right = t.right;

	// Visual Moving
	int x = left.getGuiX();
	moveTo(left, x + t.w);
	t.initX = x;

	// Logical moving
	if (left2 != NULL)
	{
		left2.right = t;
	}
	else
	{
		firstTab = t;
	}
	
	t.left = left2;

	left.right = right;
	if (right != NULL)
	{
		right.left = left;		
	}
	else
	{
		lastTab = left;
	}
	

	left.left = t;
	t.right = left;

	debugTabs();
}

moveRight (Tab t)
{
	Tab left = t.left;
	Tab right = t.right;
	Tab right2 = right.right;

	// Visual Moving
	int x = right.getGuiX();
	moveTo(right, t.initX);
	t.initX += right.w;

	// Logical moving
	if (right2 != NULL)
	{
		right2.left = t;
	}
	else
	{
		lastTab = t;
	}
	t.right = right2;

	right.left = left;
	if (left != NULL)
	{
		left.right = right;		
	}
	else
	{
		firstTab = right;
	}
	

	right.right = t;
	t.left = right;

	debugTabs();
}

moveTo (Tab g, int x)
{
	g.cancelTarget();
	g.setTargetX(x);
	g.setTargetW(g.w);
	g.setTargetSpeed(0.4);
	g.gotoTarget();
}

removeTab(Tab t)
{
	if (t.removed)
	{
		return;
	}

	t.removed = true;
	
	t.hide();
	totalTabWidth -= t.maxW;

	Tab d = t.left;
	if (d != null)
	{
		d.right = t.right;
	}
	else
	{
		firstTab = t.right;
	}

	d = t.right;
	if (d != null)
	{
		d.left = t.left;
		d.initX = t.getGuiX();
		d.setXmlParam("x", integerToString(t.getGuiX()));
		forceAlign(d);
		//jacqueline
	}
	else
	{
		lastTab = t.left;
	}
	//forceAlign(firstTab);
	//alignByResize();
	
	

	//TODO align if tabs are not all visible!

	hiddenTabs.addItem(t);

	debugTabs();
}

/**
 * Call this one everytime the tab text has changed.
 */
updateTabWidth (Tab t)
{
	totalTabWidth -= t.maxW;
	//t.w = t.findObject("cpro.tab.text").getAutoWidth() +23;
	t_temp = t.findObject("cpro.tab.text");
	t.w = t_temp.getTextWidth() +23;

	t.maxW = t.w;
	totalTabWidth += t.maxW;
	t.mid = t.w/2;
	t.setXmlParam("w", integerToString(t.w));
}

forceAlign (Tab t)
{
	aligned = false;
	align(t);
}

align(Tab t)
{
	boolean startFNbool = isFullNames;
	
	//if(isFullNames){
		if (tabHolder.getWidth() < totalTabWidth){
			aligned = false;
			alignByResize();
		}
		else if (!aligned)	{		
			alignFull(firstTab);
			aligned = true;
		}
	/*}
	else{
		if (tabHolder.getWidth() < totalTabWidth)
		{
			aligned = false;
			alignByResize();
		}
		else if(tabHolder.getWidth()>tabCount*50){
			isFullNames = true;
			alignByResize();
		}
		else if (!aligned)
		{		
			alignFull(firstTab);
			aligned = true;
		}
	}
	
	if(startFNbool!=isFullNames){
		align(t);
	}*/
}

/**
 * Recursiv align of right hand side buttons
 */
alignFull (Tab t)
{
	if (t == NULL)
		return;

	t.w = t.maxW;
	t.mid = t.maxW/2;
	t.setXmlParam("w", integerToString(t.maxW));

	Tab right = t.right;
	if (right == NULL)
		return;
	right.setXmlParam("x", integerToString(t.maxW + t.getGuiX()));
	alignFull (right);
}

/**
 * Resizes tabs so we do not run out of space
 */
 
/* OLD CODE
alignByResize ()
{
	Tab t = firstTab;

	float ratio = tabHolder.getWidth()/totalTabWidth;
	int x = 0;

	while (t != NULL)
	{
		t.w = t.maxW*ratio;
		t.mid = t.w/2;
		
		t.setXmlParam("x", integerToString(x));
		t.setXmlParam("w", integerToString(t.w));
		x+=t.w;
		t = t.right;
	}
}*/

updateTabCount(){
	
	Tab t = firstTab;
	tabCount = 0;
	while (t != NULL)	{
		tabCount++;
		t = t.right;
	}
	
	//tabCount = 5;
}

alignByResize ()
{
	Tab t = firstTab;
	
	/*
	Martin... please replace this peice of code...if possible... just used to count how many visible tabs are there... theres probably a better way to detect ;)
	*/
	updateTabCount();
	
	t = firstTab;
	
	float ratio = (tabHolder.getWidth()-tabCount*25)/(totalTabWidth-tabCount*25);
	
	text tn = t.findObject("cpro.tab.text");
	//debugstring(floattostring(ratio,5),9);
	
	/*if(isFullNames){
		if(ratio<0.65) isFullNames = false;
	}*/
	
	//disabled for cpro2
	//if(tabHolder.getWidth()<=tabCount*50) isFullNames = false;
	
	/*else{
		if(tabHolder.getWidth()>300) isFullNames = true;
	}*/
	
	int x = 0;

	/*while (t != NULL)
	{
		text tn = t.findObject("cpro.tab.text");
		//if(isFullNames){
			//text tn = t.findObject("cpro.tab.text");
			tn.setXmlParam("text", t.nameLong);
		//}
		//else{ 
			//text tn = t.findObject("cpro.tab.text");
			//tn.setXmlParam("text", t.nameShort);
		//}
		updateTabWidth(t);
		t = t.right;
	}*/

	ratio = (tabHolder.getWidth()-tabCount*25)/(totalTabWidth-tabCount*25);
	if(ratio<0) ratio=0;
	if(ratio>1) ratio=1;

	t = firstTab;
	while (t != NULL)
	{
		t.w = (t.maxW-25)*ratio+25;
		t.mid = t.w/2;
		
		t.setXmlParam("x", integerToString(x));
		t.setXmlParam("w", integerToString(t.w));
		x+=t.w;
		t = t.right;
	}
}

closeTab (tab t)
{

	//if (lastActive)
	//{
		//Deselect all tabs ** Fix for some tabs staying open - Also moved the enable tab function after the sendmessage was executed in the CProTabButton
		Tab t1 = firstTab;
		while (t1 != NULL)	{
			lastActive = t1;
			t1 = t1.right;
			lastActive.setActivated(0);
		}
		
		
		//lastActive = t;
		//lastActive.setActivated(0);
		if(lastActiveT.ID == 5 || (getPublicInt("cpro2.One.TabAutoClose."+integerToString(t.ID), 0) && t.ID != WIDGET_TAB_ID) || (t.ID==WIDGET_TAB_ID && getPublicInt("cpro2.One.TabAutoClose."+t.IDS, 0)))
		{
			removeTab(t);
		}
		alignByResize();
	//}
}

sg.onAction (String action, String param, int x, int y, int p1, int p2, GuiObject source)
{
	if(strlower(action) == "select_tab" || strlower(action) == "show_widget") // first one is msg from centro, second one from widget manager
	{
		if (strlower(action) == "show_widget")
		{
			x = WIDGET_TAB_ID;
		}
		
		//debugint(x);
		if(!checkedBrowser){
			CproBrowser = getContainer("main").getLayout("normal").findObject("centro.browser");
			//sui_browser_attrib.setData(integerToString(CproBrowser.isVisible())); //pjn123 - dont know why I put this here.. removed.. dont crash anymore
			checkedBrowser=true;
		}

		//Tab t; //pjn123 - changed this
		Tab t = firstTab;
		boolean found = false;

		//if(t.ID != x)
		//{
			if (lastActiveT)
			{
				closeTab(lastActiveT);				
			}
			//debugstring(integerToString(lastActiveT.ID),9);

			//t = firstTab; //pjn123 - changed this
			while (t != NULL)
			{
				if (t.ID == x && ((x == WIDGET_TAB_ID && param == t.IDS) || x != WIDGET_TAB_ID))
				{
					
					lastActive = lastActiveT = t;
					lastActive.setActivated(1);
					found = true;
					break;
				}
				
				t = t.right;
			}
		//}
		if (!found)
		{
			for ( int i = 0; i < hiddenTabs.getNumItems(); i++ )
			{
				t = hiddenTabs.enumItem(i);
				if (t.ID == x && ((x == WIDGET_TAB_ID && param == t.IDS) || x != WIDGET_TAB_ID))
				{
					lastActive = lastActiveT = t;
					lastActive.setActivated(1);

					t.left = lastTab; // may be null here, but is OK
					t.right = null;

					// (mpdeimos) lastTab can be NULL here
					// fixes bug from joebloggscity -- http://forums.skinconsortium.com/index.php?page=Thread&threadID=1256
					if (lastTab)
					{
						lastTab.right = t;
						t.setXmlParam("x", integerToString(lastTab.getGuiX() + lastTab.w));
					}
					else
					{
						t.setXmlParam("x", "0");
					}
					
					t.show();
					t.removed = false;
					alignByResize();

					lastTab = t;

					totalTabWidth += t.maxW;
					forceAlign(firstTab);

					hiddenTabs.removeItem(i);
					CproSUI.sendAction ("show_tab", t.IDS, t.ID, 0, 0, 0); // notif centro sui once again!
					break;
				}			
			}
		}


		if (found && strlower(action) == "show_widget")
		{
			// show sui if needed!
			layout main_normal = sg.getparentLayout();
			main_normal.getContainer().switchToLayout("normal");
			if (main_normal.getHeight() < 338)
			{
				setPublicInt("cPro2.h",388); //check this asap Note to myself (Pjn123)
				main_normal.resize(main_normal.getLeft(), main_normal.getTop(), main_normal.getWidth(), 388);
			}

			CproSUI.sendAction ("show_tab", t.IDS, t.ID, 0, 0, 0); // We need to notify centro sui
		}
		

		if(found && t.ID==WIDGET_TAB_ID)
		{
			CproSUI.sendAction ("widget_statusbar", "", t.statusbar, 0, 0, 0);
		}
	}
	else if(strlower(action) == "update_tabname")
	{	
		Tab tabI = firstTab;
		while (tabI != null)
		{
			if (tabI.ID == x)
			{
				tabI.nameLong = getToken(param, ";", 0);
				if(getToken(param, ";", 1)!="") tabI.nameShort = strupper(strLeft(getToken(param, ";", 1),3)); //Force all languagepacks to 3max & uppercase
				else tabI.nameShort = strupper(strLeft(getToken(param, ";", 0),3));
				
				Text t = tabI.findObject("cpro.tab.text");
				Layer l = tabI.findObject("cpro.tab.icon");
				//if(isFullNames) 
				t.setXmlParam("tooltip", tabI.nameLong);
				if(getToken(param, ";", 1)==QUE_MAN){
					l.setXmlParam("tooltip", "cpro.tab.icon.que");
					if(more_icons) l.setXmlParam("image", "cpro.tab.icon.que.1");
					else l.setXmlParam("image", "cpro.tab.icon.que");
				}
				else if(strright(getToken(param, ";", 1),29)=="-0000-00FF-8000-C5E2FB8CD50B}"){
					l.setXmlParam("tooltip", "cpro.tab.icon.onl");
					if(more_icons) l.setXmlParam("image", "cpro.tab.icon.onl.1");
					else l.setXmlParam("image", "cpro.tab.icon.onl");
				}
				else{
					l.setXmlParam("tooltip", "cpro.tab.icon.plu");
					if(more_icons) l.setXmlParam("image", "cpro.tab.icon.plu.1");
					else l.setXmlParam("image", "cpro.tab.icon.plu");
				}
	
	
			

				//else t.setXmlParam("text", tabI.nameShort);
				
				//JAM - Add icon update!!!
				
				//We want the fullname for the tooltip ;)
				ToggleButton t2 = tabI.findObject("cpro.tab.button");
				t2.setXmlParam("tooltip", tabI.nameLong);

				
				updateTabWidth(tabI);
				align(firstTab);
				return;
			}
			tabI = tabI.right;
		}
		
			
					
					/*text t = tabI.findObject("cpro.tab.text");
					t.setXmlParam("text", param);
					int oldTotalTabWidth = totalTabWidth;
					updateTabWidth(tabI);

					if (sg.getWidth() < totalTabWidth || sg.getWidth() < oldTotalTabWidth)
					{
						alignByResize();
					}
					else
					{
						align(tabI);	
					}
					return;*/

		/*Tab tabI = firstTab;
		while (tabI != null)
		{
			if (tabI.isInternal)
			{
				if (tabI.ID == x)
				{
					text t = tabI.findObject("cpro.tab.text");
					t.setXmlParam("text", param);
					int oldTotalTabWidth = totalTabWidth;
					updateTabWidth(tabI);

					if (sg.getWidth() < totalTabWidth || sg.getWidth() < oldTotalTabWidth)
					{
						alignByResize();
					}
					else
					{
						align(tabI);	
					}
					return;
				}
			}
			tabI = tabI.right;
		}*/
	}
	
	debugTabs();
}


CproTabAtt.onDataChanged(){
	if(CproTabAtt.getData()=="1")
	{
		CproSUI.sendAction ("switch_to_tab", CproTabAtt.IDS, WIDGET_TAB_ID, 0, 0, 0);
		CproTabAtt.setData("0");
	}
}

sui_browser_attrib.onDataChanged(){
	if(sui_browser_attrib.getData()=="1")
	{
		CproSUI.sendAction ("switch_to_tab", "", 4, 0, 0, 0);
	}
	else if(sui_browser_attrib.getData()=="0" && CproBrowser.isVisible()){
		CproSUI.sendAction ("switch_to_tab", "", 0, 0, 0, 0); // remember to add the planned future goto history tab here too! atm is just goto the lib
	}
}

CproBrowser.onSetVisible(boolean onOff){
	if(onOff) sui_browser_attrib.setData("1");
	else sui_browser_attrib.setData("0");
}
System.onKeyDown(String key) {
	if (key == "alt+x")
	{
		if (sui_browser_attrib.getData() == "0") sui_browser_attrib.setData("1");
		else sui_browser_attrib.setData("0");
		complete;
	}
}

#ifdef DEBUG

debugTabs()
{
	Tab t = firstTab;
	while (t != null)
	{
		debugTab(t);
		t = t.right;
	}
}

debugTab (Tab t)
{
	if (t.left != NULL)
	{
		Tab d = t.left;
		t.findObject("l").setXmlParam("text", (d.findObject("cpro.tab.text").getXmlParam("text")));
	}
	else
	{
		t.findObject("l").setXmlParam("text", "~");
	}

	if (t.right != NULL)
	{
		Tab d = t.right;
		t.findObject("r").setXmlParam("text", (d.findObject("cpro.tab.text").getXmlParam("text")));
	}
	else
	{
		t.findObject("r").setXmlParam("text", "~");
	}
}
#endif


Function cProLoaded();

/* Notify Widget Manager, what cool widgets has been installed */ 
StartupCallback.onLoaded()
{
	cProLoaded();
}

/** somehow winamp creates PVC calls when i do this stuff above */
cProLoaded ()
{
	Layout main_normal = sg.getparentLayout();
	int widgetPlace = main_normal.onAction("widget_manager_register", "Main Area", 0,0,0,0,sg); // TODO Translate
	for (int i = 0; i < widgetLoader.getNumChildren(); i++)
	{
		GuiObject d = widgetLoader.enumChildren(i);
		main_normal.onAction("widget_manager_check", getToken(d.getXmlParam("userdata"), ";", 0), widgetPlace,0,0,0,d);
	}
	widgetPlace = main_normal.onAction("widget_manager_done", "", widgetPlace,0,0,0,sg);
}